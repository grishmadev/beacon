use std::error::Error;
use std::ffi::CString;
use std::fs::{self, write};
use std::io;
use std::mem::MaybeUninit;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixDatagram;
use std::time::{Duration, Instant};

use dhcp4r::options::{DhcpOption, MessageType};
use dhcp4r::packet::Packet;
use rtnetlink::{Handle, new_connection};
use socket2::{Domain, Protocol, SockAddr, Type};

use crate::HISTORY_PATH;
use crate::types::{Connection, DhcpLease};

pub async fn connect(
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
        format!(
            "wpa_supplicant not running. Start it first:\n  
                       sudo wpa_supplicant -B -i {} -c /etc/wpa_supplicant.conf",
            ifname
        )
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
    let host_data = request_host_data(ifindex, ifname, mac_address)?;
    println!("host data: {:#?}", host_data);

    send_dhcp_request(&host_data.offer, host_data.ip_addr.unwrap(), ifname)?;

    let (connection, handle, _) = new_connection()?;

    tokio::spawn(connection);

    apply_network_config(
        handle,
        *ifindex,
        host_data.ip_addr.unwrap(),
        host_data.gateway.unwrap(),
    )
    .await?;

    set_dns(host_data.dns_servers)?;
    // tokio::signal::ctrl_c().await?;
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

async fn apply_network_config(
    handle: Handle,
    ifindex: u32,
    ip: Ipv4Addr,
    gateway: Ipv4Addr,
) -> Result<(), Box<dyn Error>> {
    // Add IP address (/24)
    handle
        .address()
        .add(ifindex, ip.into(), 24)
        .execute()
        .await?;

    // set the interface up
    handle.link().set(ifindex).up().execute().await?;

    // Add default route
    handle
        .route()
        .add()
        .v4()
        .destination_prefix(Ipv4Addr::UNSPECIFIED, 0)
        .gateway(gateway)
        .execute()
        .await?;

    Ok(())
}

fn set_dns(dns_servers: Vec<Ipv4Addr>) -> Result<(), Box<dyn Error>> {
    if dns_servers.is_empty() {
        return Ok(());
    }
    let mut config_lines = Vec::<String>::new();
    for dns in dns_servers {
        config_lines.push(format!("nameserver {}", dns));
    }
    println!("dns: {}", config_lines.join("\n"));
    match write("/etc/resolv.conf", config_lines.join("\n")) {
        Ok(_) => {
            println!("DNS set!")
        }
        Err(e) => {
            return Err(e.into());
        }
    };
    Ok(())
}

fn send_dhcp_request(
    offer: &Packet,
    offered_ip: Ipv4Addr,
    ifname: &str,
) -> Result<(), Box<dyn Error>> {
    let mut options = Vec::new();

    // message type requesst
    options.push(DhcpOption::DhcpMessageType(MessageType::Request));
    println!("offered ip address: {:?}", offer.yiaddr);

    // requested ip address (optins 50)
    options.push(DhcpOption::RequestedIpAddress(offered_ip));

    // options 54
    if let Some(server_id) = offer.options.iter().find_map(|o| {
        if let DhcpOption::ServerIdentifier(id) = o {
            Some(*id)
        } else {
            None
        }
    }) {
        options.push(DhcpOption::ServerIdentifier(server_id));
    }

    // ask for same things as before
    options.push(DhcpOption::ParameterRequestList(vec![1, 3, 6, 15, 51]));

    let request_packet = Packet {
        reply: false,
        hops: 0,
        xid: offer.xid,
        secs: 0,
        broadcast: true,
        ciaddr: Ipv4Addr::UNSPECIFIED,
        yiaddr: Ipv4Addr::UNSPECIFIED,
        siaddr: Ipv4Addr::UNSPECIFIED,
        giaddr: Ipv4Addr::UNSPECIFIED,
        chaddr: offer.chaddr,
        options,
    };

    // bind socket to 0.0.0.0 because we dont yet have an IP
    let socket = UdpSocket::bind("0.0.0.0:68")?;

    // allow broadcasting
    socket.set_broadcast(true)?;

    bind_socket_to_device(&socket, ifname)?;

    let dest = "255.255.255.255:67";

    let mut buf = [0u8; 1500];
    let data = request_packet.encode(&mut buf);

    socket.send_to(data, dest)?;

    println!(
        "DHCPREQUESST msg sent for IP: {:?}",
        request_packet.options.iter().find_map(|opt| {
            if let dhcp4r::options::DhcpOption::RequestedIpAddress(ip) = opt {
                Some(ip)
            } else {
                None
            }
        })
    );

    Ok(())
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

fn request_host_data(
    ifindex: &u32,
    ifname: &str,
    mac_address: [u8; 6],
) -> Result<DhcpLease, Box<dyn Error>> {
    println!("checkpoint");
    // point it at global broadcast address
    // socket used only for sending signals
    let broadcast_addr: SocketAddr = "255.255.255.255:67".parse().unwrap();
    let total_retries = 5;
    let xid = rand::random();
    let send_socket = UdpSocket::bind("0.0.0.0:68")?;
    send_socket.set_broadcast(true)?;
    bind_socket_to_device(&send_socket, ifname)?;

    let socket = socket2::Socket::new(
        Domain::from(libc::AF_PACKET),
        Type::from(libc::SOCK_RAW),
        Some(Protocol::from(libc::ETH_P_IP)),
    )?;
    let sockaddr = unsafe {
        let mut ll: libc::sockaddr_ll = std::mem::zeroed();
        ll.sll_family = libc::AF_PACKET as u16;
        ll.sll_ifindex = *ifindex as i32;
        ll.sll_protocol = (libc::ETH_P_IP as u16).to_be(); // 0x800
        //
        let mut storage: libc::sockaddr_storage = std::mem::zeroed();
        std::ptr::copy_nonoverlapping(
            &ll as *const libc::sockaddr_ll as *const u8,
            &mut storage as *mut libc::sockaddr_storage as *mut u8,
            std::mem::size_of::<libc::sockaddr_ll>(),
        );

        // wrapping in socket2 addr
        SockAddr::new(
            std::mem::transmute_copy(&storage),
            std::mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t,
        )
    };
    socket.bind(&sockaddr)?;

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
                DhcpOption::ParameterRequestList(vec![2, 3, 6, 15, 51]), // Subnet, Router, DNS, Domain
            ],
        };

        let mut buf = [0u8; 1500];
        let slice = msg.encode(&mut buf);

        println!("checkpoint 2");
        // sending the socket
        send_socket.send_to(slice, broadcast_addr)?;

        socket.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
        let mut res_buf = [MaybeUninit::<u8>::zeroed(); 1500];
        let timeout = Instant::now() + Duration::from_secs(3);

        let mut subnet_mask: Option<Ipv4Addr> = None;
        let mut ip_addr: Option<Ipv4Addr> = None;
        let mut gateway: Option<Ipv4Addr> = None;
        let mut dns_servers: Option<Vec<Ipv4Addr>> = None;
        let mut lease_duration = 0u32;
        let mut server_id: Option<Ipv4Addr> = None;
        loop {
            let now = Instant::now();
            if now >= timeout {
                break;
            }
            match socket.recv_from(&mut res_buf) {
                Ok((size, _)) => {
                    let initialized_data =
                        unsafe { std::slice::from_raw_parts(res_buf.as_ptr() as *const u8, size) };

                    if size < 42 {
                        continue;
                    }
                    if initialized_data[23] != 17 {
                        continue;
                    }
                    let dest_port =
                        u16::from_be_bytes([initialized_data[36], initialized_data[37]]);
                    if dest_port != 68 {
                        continue;
                    }
                    if initialized_data[42] != 2 {
                        continue;
                    }
                    println!("found right packet");

                    // skipping ethernet, ip, udp headers
                    let dhcp_data = &initialized_data[42..];
                    let packet =
                        Packet::from(dhcp_data).map_err(|_| "Failed to parse DHCP Packet.")?;

                    println!("packet xid: {:?}, msg xid: {:?}", packet.xid, msg.xid);

                    if packet.xid != msg.xid {
                        continue;
                    }

                    for option in packet.options {
                        // checking if offer answered
                        match option {
                            DhcpOption::DhcpMessageType(val) => match val {
                                dhcp4r::options::MessageType::Offer => {
                                    println!("Offered IP: {:?}", packet.yiaddr);
                                    ip_addr = Some(packet.yiaddr);
                                }
                                _ => {
                                    println!("Didnt find desired message.");
                                }
                            },
                            DhcpOption::DomainNameServer(ips) => dns_servers = Some(ips),
                            DhcpOption::Router(routers) => {
                                if !routers.is_empty() {
                                    gateway = Some(routers[0]);
                                }
                            }
                            DhcpOption::SubnetMask(subnet) => subnet_mask = Some(subnet),
                            DhcpOption::ServerIdentifier(id) => server_id = Some(id),
                            DhcpOption::IpAddressLeaseTime(secs) => lease_duration = secs,
                            _ => {}
                        };
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
            let result: DhcpLease = DhcpLease {
                ip_addr,
                subnet_mask,
                dns_servers: dns_servers.unwrap_or(vec![]),
                server_id,
                lease_duration,
                renewal_time: lease_duration / 2,
                rebinding_time: (lease_duration as f64 * 0.875) as u32,
                gateway,
                offer: msg,
            };
            return Ok(result);
        }
        println!(
            "No reply, Retrying... Attempts left {}",
            total_retries - attempt
        );
    }
    Err("Failed after retry.".into())
}
