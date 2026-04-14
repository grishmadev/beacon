use std::error::Error;
use std::ffi::CString;
use std::io::{self, ErrorKind};
use std::mem::MaybeUninit;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixDatagram;
use std::time::{Duration, Instant};

use dhcp4r::options::DhcpOption;
use dhcp4r::packet::Packet;
use socket2::{Domain, Protocol, SockAddr, Type};

pub fn connect(
    mac_address: [u8; 6],
    ifname: &str,
    ifindex: &u32,
    ssid: &str,
    password: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let server_path = format!("/var/run/wpa_supplicant/{}", ifname);
    let client_path = format!("/tmp/beacon_{}", std::process::id());

    // remove client socket if already exists
    let _ = std::fs::remove_file(&client_path);

    // bind socket to client path
    let skt = UnixDatagram::bind(&client_path)?;

    // connect to server
    skt.connect(&server_path).map_err(|_| {
        "wpa_supplicant not running. Start it first:\n  \
                       sudo wpa_supplicant -B -i wlo1 -c /etc/wpa_supplicant.conf"
    })?;

    let send_cmd = |cmd: &str| -> Result<String, Box<dyn Error>> {
        println!("sending cmd: {}", cmd);

        skt.send(cmd.as_bytes())?;

        let mut buf = [0u8; 1024 * 4];
        while let Ok(size) = skt.recv(&mut buf) {
            let reply = String::from_utf8_lossy(buf[..size].into())
                .trim()
                .to_string();
            if reply.starts_with('<') {
                continue;
            }
            println!("result: {}", reply);
            return Ok(reply);
        }
        Ok("FAIL".to_string())
    };

    if send_cmd("PING")? != "PONG" {
        return Err("wpa_supplicant did not respond.".into());
    }

    // remove any previous network
    let _ = send_cmd("REMOVE_NETWORK all");

    if send_cmd("ATTACH")? != "OK" {
        return Err("Couldn't connect to wpa_supplicant.".into());
    }

    //set socket to non blocking
    let _ = skt.set_nonblocking(true);
    // drain socket
    let mut tmp = [0u8; 4096];
    while skt.recv(&mut tmp).is_ok() {}

    // switch to blocking
    let _ = skt.set_nonblocking(false);

    // add network
    // check if password exists
    let network_id = if let Some(pass) = password {
        let network_id = send_cmd("ADD_NETWORK")?;
        // .split(" ")
        // .last()
        // .unwrap_or("")
        // .to_string();

        let ssid_cmd = format!("SET_NETWORK {} ssid \"{}\"", network_id, ssid);
        let ssid_ok = send_cmd(&ssid_cmd)?;
        if ssid_ok != "OK" {
            return Err(format!("failed to set SSID. {}", ssid_ok).into());
        }

        // set psk
        let psk_ok = send_cmd(&format!("SET_NETWORK {} psk \"{}\"", network_id, pass))?;
        if psk_ok != "OK" {
            return Err(format!("failed to set password. {}", psk_ok).into());
        }

        send_cmd("SAVE_CONFIG")?;
        network_id
    } else {
        println!("Password not provided. Checking for saved hosts.");
        match find_saved_networks(&send_cmd, ssid)? {
            Some(s) => s,
            None => return Err("Password not provided for new host.".into()),
        }
    };

    println!("found network: {}", network_id);

    // disable other networks incase wpa_supplicant connects to any cached network
    let disable_ok = send_cmd("DISABLE_NETWORK all")?;
    if disable_ok != "OK" {
        return Err(format!("Couldn't disable cached networks. {}", disable_ok).into());
    }

    let select_ok = send_cmd(&format!("SELECT_NETWORK {}", network_id))?;
    if select_ok != "OK" {
        return Err(format!("Couldn't connect to {}. {}", ssid, select_ok).into());
    }

    println!("Connecting to {}..", ssid);
    let mut recv_buffer = [0u8; 1024 * 4];

    loop {
        skt.set_read_timeout(Some(std::time::Duration::from_secs(100)))?;

        match skt.recv(&mut recv_buffer) {
            Ok(size) => {
                let event = String::from_utf8_lossy(&recv_buffer[..size])
                    .trim()
                    .to_string();
                println!("event: {}", event);
                if event.contains("CTRL-EVENT-CONNECTED") {
                    request_ip(ifindex, mac_address)?;
                    println!("Connected.");
                    break;
                } else if event.contains("CTRL-EVENT-AUTH-REJECT") {
                    send_cmd(&format!("REMOVE_NETWORK {}", network_id))?;
                    send_cmd("SAVE_CONFIG")?;
                    return Err("Authentication failed. Try Again.".into());
                } else if event.contains("CTRL-EVENT-NETWORK-NOT-FOUND") {
                    return Err("Network not found, Make sure host is in range.".into());
                }
            }
            Err(_) => return Err("connection timed out after 10 secs.".into()),
        }
    }

    Ok(())
}

pub fn disconnect(ifname: &str) -> Result<(), Box<dyn Error>> {
    let server_path = format!("/var/run/wpa_supplicant/{}", ifname);
    let skt = UnixDatagram::bind(&server_path)?;

    skt.connect(&server_path)
        .map_err(|_| "wpa_supplicant not running or Wifi is turned off.")?;

    skt.send("DISCONNECT".as_bytes())?;
    let mut recv_buf = [0u8; 4096];
    loop {
        // wait for 5 secs to disconnet
        skt.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
        let size = skt.recv(&mut recv_buf)?;
        let event = String::from_utf8_lossy(&recv_buf[..size])
            .trim()
            .to_string();
        if event.contains("CTRL-EVENT-DISCONNECTED") {
            return Ok(());
        };
    }
}

fn find_saved_networks(
    send_cmd: &impl Fn(&str) -> Result<String, Box<dyn Error>>,
    target_ssid: &str,
) -> Result<Option<String>, Box<dyn Error>> {
    let networks = send_cmd("LIST_NETWORKS")?;

    for line in networks.lines().skip(1) {
        let col: Vec<&str> = line.splitn(4, '\t').collect();
        if col.len() >= 2 && col[1] == target_ssid {
            return Ok(Some(col[0].to_string()));
        }
    }
    Ok(None)
}

fn bind_socket_to_device(socket: &UdpSocket, ifname: &str) -> Result<(), Box<dyn Error>> {
    let ifname = CString::new(ifname)?;
    let fd = socket.as_raw_fd();

    unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_BINDTODEVICE,
            ifname.as_ptr() as *const libc::c_void,
            ifname.as_bytes().len() as libc::socklen_t,
        );
    };

    Ok(())
}

fn request_ip(ifindex: &u32, mac_address: [u8; 6]) -> Result<(), Box<dyn Error>> {
    println!("checkpoint");
    let socket = socket2::Socket::new(
        Domain::from(libc::AF_PACKET),
        Type::from(libc::SOCK_RAW),
        Some(Protocol::from(libc::ETH_P_IP)),
    )?;
    let sockaddr = unsafe {
        let mut storage: libc::sockaddr_ll = std::mem::zeroed();
        storage.sll_family = libc::AF_PACKET as u16;
        storage.sll_ifindex = *ifindex as i32;
        storage.sll_protocol = (libc::ETH_P_IP as u16).to_be(); // 0x800

        // wrapping in socket2 addr
        SockAddr::new(
            std::mem::transmute_copy(&storage),
            std::mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t,
        )
    };
    socket.bind(&sockaddr)?;

    // point it at global broadcast address
    // let addr: SocketAddr = "255.255.255.255:67".parse().unwrap();
    let total_retries = 5;
    let xid = rand::random();
    for attempt in 0..total_retries {
        let msg = Packet {
            reply: false,
            hops: 0,
            xid,
            secs: 0,
            broadcast: true,
            ciaddr: Ipv4Addr::UNSPECIFIED,
            yiaddr: Ipv4Addr::UNSPECIFIED,
            siaddr: Ipv4Addr::UNSPECIFIED,
            giaddr: Ipv4Addr::UNSPECIFIED,
            chaddr: mac_address,
            options: vec![
                DhcpOption::DhcpMessageType(dhcp4r::options::MessageType::Discover),
                DhcpOption::ParameterRequestList(vec![1, 3, 6, 15]), // Subnet, Router, DNS, Domain
            ],
        };

        // let mut buf = [0u8; 1500];
        // let slice = msg.encode(&mut buf);

        println!("checkpoint 2");
        // sending the socket
        // socket.send_to(slice, addr)?;

        // socket.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
        // let mut res_buf = [0u8; 1500];
        let mut res_buf = [MaybeUninit::<u8>::zeroed(); 1500];
        let timeout = Instant::now() + Duration::from_secs(10);

        loop {
            let now = Instant::now();
            if now >= timeout {
                break;
            }
            match socket.recv_from(&mut res_buf) {
                Ok((size, src)) => {
                    // println!("got {} bytes from {}", size, src);

                    println!("src data: {:?}", src);
                    let initialized_data =
                        unsafe { std::slice::from_raw_parts(res_buf.as_ptr() as *const u8, size) };

                    if size < 42 {
                        continue;
                    }

                    // skipping ethernet, ip, udp headers
                    let dhcp_data = &initialized_data[42..];

                    let packet =
                        Packet::from(dhcp_data).map_err(|_| "Failed to parse DHCP Packet.")?;

                    println!("packet xid: {:?}, msg xid: {:?}", packet.xid, msg.xid);

                    if packet.xid != msg.xid {
                        continue;
                    }
                    for option in packet.options {
                        // checking if offer answered and printing offered IP address
                        if let DhcpOption::DhcpMessageType(val) = option {
                            match val {
                                dhcp4r::options::MessageType::Offer => {
                                    println!("Offered IP: {:?}", packet.yiaddr);
                                    return Ok(());
                                }
                                dhcp4r::options::MessageType::Ack => {
                                    println!("got ACK: {}", packet.yiaddr);
                                    break;
                                }
                                _ => {
                                    println!("Didnt find desired message.");
                                }
                            }
                        }
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // no packets keep waiting.
                    // println!("Getting WouldBlock Errors");
                    continue;
                }
                Err(e) => {
                    println!("Kernel Error: {:?}", e);
                    return Err(e.into());
                }
            }
        }
        println!(
            "No reply, Retrying... Attempts left {}",
            total_retries - attempt
        );
    }
    Err("Failed after retry.".into())
}
