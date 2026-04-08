use std::error::Error;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::os::unix::net::UnixDatagram;

use dhcp4r::options::DhcpOption;
use dhcp4r::packet::Packet;

pub fn connect(
    mac_address: [u8; 6],
    ifname: &str,
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
                    request_ip(mac_address)?;
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

fn request_ip(mac_address: [u8; 6]) -> Result<(), Box<dyn Error>> {
    println!("checkpoint");
    let socket = UdpSocket::bind("0.0.0.0:68")?;
    socket.set_broadcast(true)?;

    // point it at global broadcast address
    let addr: SocketAddr = "255.255.255.255:67".parse().unwrap();
    let msg = Packet {
        reply: false,
        hops: 0,
        xid: rand::random(),
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

    let mut buf = [0u8; 1500];
    let slice = msg.encode(&mut buf);

    println!("checkpoint 2");
    // sending the socket
    socket.send_to(slice, addr)?;

    let mut res_buf = [0u8; 1500];

    loop {
        let size = socket.recv(&mut res_buf)?;
        let packet = Packet::from(&res_buf[..size]).map_err(|_| "Failed to parse DHCP Packet.")?;
        if packet.xid != msg.xid {
            println!("found: {:?}", packet.reply);
            continue;
        }
        for option in packet.options {
            match option {
                DhcpOption::Router(gateways) => {
                    println!("Router IPs: {:?}", gateways);
                }
                DhcpOption::SubnetMask(mask) => {
                    println!("subnet mast: {:?}", mask);
                }
                _ => {}
            }
        }
        return Ok(());
    }
}
