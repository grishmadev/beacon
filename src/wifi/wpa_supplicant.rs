use std::any::Any;
use std::error::Error;
use std::ffi::CString;
use std::fs::{self, write};
use std::io::{self, Cursor};
use std::mem::MaybeUninit;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::os::fd::AsRawFd;
use std::os::raw;
use std::os::unix::net::UnixDatagram;
use std::thread;
use std::time::{Duration, Instant};

use dhcp4r::options::{DhcpOption, MessageType, RawDhcpOption};
use dhcp4r::packet::Packet;
use libc::sleep;
use neli::attr::Attribute;
use neli::consts::nl::NlmF;
use neli::consts::rtnl::{RtAddrFamily, RtScope, RtTable, Rta, Rtm, Rtn, Rtprot};
use neli::consts::socket::{Msg, NlFamily};
use neli::nl::{NlPayload, Nlmsghdr, NlmsghdrBuilder};
use neli::rtnl::{Rtmsg, RtmsgBuilder};
use neli::socket::NlSocket;
use neli::types::RtBuffer;
use neli::utils::Groups;
use neli::{FromBytes, ToBytes, router};
use rtnetlink::{Handle, new_connection};
use socket2::{Domain, Protocol, SockAddr, SockAddrStorage, Socket, Type, sa_family_t};

use crate::backend::functions::list_interfaces;
use crate::debug::write as cwrite;
use crate::types::{DhcpLease, Interface};
use crate::wifi::helper::{
    create_packet_sockaddr, generate_client_id, get_interfaces, validate_packet, validate_packet_v2,
};

pub async fn connect(
    mac_address: [u8; 6],
    ifname: &str,
    ifindex: &u32,
    ssid: &str,
    password: &str,
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
        skt.send(cmd.as_bytes())?;

        let mut buf = [0u8; 1024 * 4];
        while let Ok(size) = skt.recv(&mut buf) {
            let reply = String::from_utf8_lossy(buf[..size].into())
                .trim()
                .to_string();
            if reply.starts_with('<') {
                continue;
            }
            cwrite(format!("result: {}", reply));
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
    let network_id = {
        let network_id = send_cmd("ADD_NETWORK")?;

        let ssid_cmd = format!("SET_NETWORK {} ssid \"{}\"", network_id, ssid);
        let ssid_ok = send_cmd(&ssid_cmd)?;
        if ssid_ok != "OK" {
            return Err(format!("failed to set SSID. {}", ssid_ok).into());
        }

        // set psk
        let psk_ok = send_cmd(&format!("SET_NETWORK {} psk \"{}\"", network_id, password))?;
        if psk_ok != "OK" {
            return Err(format!("failed to set password. {}", psk_ok).into());
        }

        send_cmd("SAVE_CONFIG")?;
        network_id
    };
    cwrite(format!("found network: {}", network_id));

    // disable other networks incase wpa_supplicant connects to any cached network
    let disable_ok = send_cmd("DISABLE_NETWORK all")?;
    if disable_ok != "OK" {
        return Err(format!("Couldn't disable cached networks. {}", disable_ok).into());
    }

    let select_ok = send_cmd(&format!("SELECT_NETWORK {}", network_id))?;
    if select_ok != "OK" {
        return Err(format!("Couldn't connect to {}. {}", ssid, select_ok).into());
    }

    cwrite(format!("Connecting to {}..", ssid));
    let mut recv_buffer = [0u8; 1024 * 4];

    loop {
        skt.set_read_timeout(Some(std::time::Duration::from_secs(100)))?;

        match skt.recv(&mut recv_buffer) {
            Ok(size) => {
                let event = String::from_utf8_lossy(&recv_buffer[..size])
                    .trim()
                    .to_string();
                cwrite(format!("event: {}", event));
                if event.contains("CTRL-EVENT-CONNECTED") {
                    cwrite("Connected.".into());
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
    // Discover packet sent here
    let host_data = discover_host(ifindex, ifname, mac_address)?;
    cwrite(format!("host data: {:#?}", host_data));

    if let Some(offer) = &host_data.offer {
        // Request packet sent here
        send_dhcp_request(offer, host_data.ip_addr.unwrap(), ifname)?;
    }

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

pub fn find_active_interface() -> Result<Option<Interface>, Box<dyn Error>> {
    let ifaces = get_interfaces()?;
    let socket = NlSocket::connect(NlFamily::Route, None, Groups::empty())?;

    let rtmsg = RtmsgBuilder::default()
        .rtm_family(RtAddrFamily::Inet)
        .rtm_dst_len(0)
        .rtm_src_len(0)
        .rtm_tos(0)
        .rtm_table(RtTable::Main)
        .rtm_protocol(Rtprot::Unspec)
        .rtm_scope(RtScope::Universe)
        .rtm_type(Rtn::Unicast)
        .rtattrs(RtBuffer::new())
        .build()?;

    let nl_msg = NlmsghdrBuilder::default()
        .nl_flags(NlmF::DUMP | NlmF::REQUEST)
        .nl_type(Rtm::Getroute)
        .nl_payload(NlPayload::Payload(rtmsg))
        .build()?;

    let mut msg_buf = Cursor::new(Vec::<u8>::new());
    nl_msg.to_bytes(&mut msg_buf)?;

    socket.send(msg_buf.get_ref(), Msg::empty())?;

    // let mut check: Rtmsg;

    let mut recv_buf = [0u8; 4096 * 16];
    let (size, _) = socket.recv(&mut recv_buf, Msg::empty())?;
    let mut res_buf = Cursor::new(&recv_buf[..size]);
    let res: Nlmsghdr<Rtm, Rtmsg> = Nlmsghdr::from_bytes(&mut res_buf)?;

    if let NlPayload::Err(e) = res.nl_payload() {
        return Err(format!("Kernel Error: {}", e).into());
    }

    let mut ifindex: Option<u32> = None;
    if let NlPayload::Payload(link_info) = res.nl_payload() {
        let attrs = link_info.rtattrs();
        for attr in attrs.iter() {
            // let res_buf = attr.rta_payload();
            match attr.rta_type() {
                Rta::Table => {
                    let table = attr.get_payload_as::<u8>()?;
                    // cwrite("table: {:?}", table);
                }
                Rta::Priority => {
                    let priority = attr.get_payload_as::<u16>()?;
                    // cwrite("priority: {:?}", priority);
                }
                Rta::Oif => {
                    ifindex = Some(attr.get_payload_as::<u32>()?);
                    // cwrite("Interface Index (OIF): {:?}", ifindex);
                }
                Rta::Gateway => {
                    let gateway = attr.get_payload_as::<[u8; 4]>()?;
                    // cwrite("Gateway IP: {:?}", gateway);
                }
                Rta::Prefsrc => {
                    let src = attr.get_payload_as::<[u8; 4]>()?;
                    // cwrite("Preferred Source IP: {:?}", src);
                }
                _ => {}
            }
        }
    }
    let result = ifaces
        .iter()
        .find(|iface| iface.ifindex == ifindex)
        .cloned();
    Ok(result)
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
    cwrite(format!("dns: {}", config_lines.join("\n")));
    match write("/etc/resolv.conf", config_lines.join("\n")) {
        Ok(_) => cwrite("DNS set!".into()),
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
    cwrite(format!("offered ip address: {:?}", offer.yiaddr));

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
        }),
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

pub fn discover_host(
    ifindex: &u32,
    ifname: &str,
    mac_address: [u8; 6],
) -> Result<DhcpLease, Box<dyn Error>> {
    // point it at global broadcast address
    // socket used only for sending signals
    let broadcast_addr: SocketAddr = "255.255.255.255:67".parse().unwrap();
    let total_retries = 5;
    let xid = rand::random();
    let send_socket = UdpSocket::bind("0.0.0.0:68")?;
    send_socket.set_broadcast(true)?;
    bind_socket_to_device(&send_socket, ifname)?;

    let socket = socket2::Socket::new(
        Domain::PACKET,
        Type::RAW,
        Some(Protocol::from(libc::ETH_P_IP)),
    )?;
    let sockaddr = create_packet_sockaddr(*ifindex);
    socket.bind(&sockaddr)?;

    for _ in 0..total_retries {
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

                    let packet =
                        validate_packet_v2(initialized_data, size)?.expect("No Packet Found.");

                    if packet.xid != msg.xid {
                        continue;
                    }

                    for option in packet.options {
                        // checking if offer answered
                        match option {
                            DhcpOption::DhcpMessageType(val) => match val {
                                dhcp4r::options::MessageType::Offer => {
                                    let _ = cwrite(format!("Offered IP: {:?}", packet.yiaddr));
                                    ip_addr = Some(packet.yiaddr);
                                }
                                _ => {
                                    let _ = cwrite("Didnt find desired message.".into());
                                }
                            },
                            DhcpOption::DomainNameServer(ips) => dns_servers = Some(ips),
                            DhcpOption::Router(routers) if !routers.is_empty() => {
                                gateway = Some(routers[0]);
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
                    // cwrite("Getting WouldBlock Errors");
                    continue;
                }
                Err(e) => {
                    // cwrite("Kernel Error: {:?}", e);
                    return Err(e.into());
                }
            }
            let result: DhcpLease = DhcpLease {
                ip_addr,
                subnet_mask,
                dns_servers: dns_servers.unwrap_or(vec![]),
                server_id,
                lease_duration,
                gateway,
                offer: Some(msg),
            };
            return Ok(result);
        }
    }
    Err("Failed after retry.".into())
}

pub fn request_host(
    mac_address: [u8; 6],
    current_ip: Ipv4Addr,
    server_id: Ipv4Addr,
    broadcast: bool,
) -> Result<DhcpLease, Box<dyn Error>> {
    let current_iface = find_active_interface()?.expect("No Active Interface Found.");
    let ifname = current_iface.ifname.expect("No Ifname found.");
    let ifindex = current_iface.ifindex.expect("No Ifindex found.");
    let socket = socket2::Socket::new(
        Domain::PACKET,
        Type::RAW,
        Some(Protocol::from(libc::ETH_P_IP)),
    )?;
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 68);
    let std_addr = SocketAddrV4::new(
        if broadcast {
            Ipv4Addr::new(255, 255, 255, 255)
        } else {
            server_id
        },
        67,
    );
    let send_socket = UdpSocket::bind(addr)?;
    // let std_addr = SocketAddrV4::new(server_ip, 67);

    let sockaddr = create_packet_sockaddr(ifindex);
    socket.bind(&sockaddr)?;
    send_socket.set_broadcast(true)?;

    bind_socket_to_device(&send_socket, &ifname)?;
    socket.bind_device(Some(ifname.as_bytes()))?;
    // let assigned_port = socket.local_addr()?;
    // println!(
    //     "Assigned port: {}",
    //     assigned_port.as_socket_ipv4().unwrap().port()
    // );

    let timeout = Instant::now() + Duration::from_secs(5);

    let duid_payload = generate_client_id(mac_address);
    let client_id = DhcpOption::Unrecognized(RawDhcpOption {
        code: 61,
        data: duid_payload,
    });
    let vendor_id = DhcpOption::Unrecognized(RawDhcpOption {
        code: 60,
        data: "beacon-0.1".as_bytes().to_vec(),
    });

    // 1500 - 20 (IP header) - 8 (UDP header)
    let msz_bytes = 1472u16.to_be_bytes().to_vec();
    let msz_option = DhcpOption::Unrecognized(RawDhcpOption {
        code: 57,
        data: msz_bytes,
    });

    let msg = Packet {
        reply: false,
        xid: rand::random(),
        ciaddr: current_ip,
        chaddr: mac_address,
        hops: 0,
        secs: 0,
        broadcast,
        yiaddr: Ipv4Addr::new(0, 0, 0, 0),
        siaddr: Ipv4Addr::new(0, 0, 0, 0),
        giaddr: Ipv4Addr::new(0, 0, 0, 0),
        options: vec![
            DhcpOption::DhcpMessageType(MessageType::Request),
            DhcpOption::ParameterRequestList(vec![1, 3, 6, 15, 51]),
        ],
    };

    let mut buf = [0u8; 1500];
    let encoded = msg.encode(&mut buf);
    send_socket.send_to(encoded, std_addr)?;
    // socket.set_read_timeout(Some(Duration::from_millis(3000)))?;
    // socket.set_write_timeout(Some(Duration::from_millis(3000)))?;
    let mut lease = DhcpLease::default();
    let mut res_buf = [MaybeUninit::<u8>::zeroed(); 1500];
    // let mut res_buf = [0u8; 4096];
    println!("Checkpoint 0");

    loop {
        thread::sleep(Duration::from_millis(300));
        let start = Instant::now();
        if start >= timeout {
            println!("Timeout");
            break;
        }
        match socket.recv(&mut res_buf) {
            Ok(size) => {
                println!("Checkpoint 1");
                let raw_data =
                    unsafe { std::slice::from_raw_parts(res_buf.as_ptr() as *const u8, size) };
                println!("raw_data: {:?}", raw_data);
                // let raw_data = &res_buf[..size];
                let packet = match validate_packet_v2(raw_data, size)? {
                    Some(s) => {
                        println!("Packet successful");
                        println!("recieved options: {:#?}", s.options);
                        s
                    }
                    None => {
                        print!("Conversion Error");
                        break;
                    }
                };
                if packet.xid != msg.xid {
                    continue;
                }
                println!(
                    "packet recieved: {:#?}, packet expected: {:#?}",
                    packet.xid, msg.xid
                );
                println!("Checkpoint 2");
                for option in packet.options {
                    match option {
                        DhcpOption::DhcpMessageType(val) => match val {
                            MessageType::Ack => {
                                println!("Server acknwoledgeeed");
                                lease.ip_addr = if packet.yiaddr.is_unspecified() {
                                    Some(current_ip)
                                } else {
                                    Some(packet.yiaddr)
                                };
                            }
                            MessageType::Nak => {
                                println!("Server Refused");
                                let _ = cwrite("Server Refused to acknwoledge.".to_string());
                            }
                            _ => {}
                        },
                        DhcpOption::DomainNameServer(ips) => lease.dns_servers = ips,
                        DhcpOption::Router(routers) if !routers.is_empty() => {
                            lease.gateway = Some(routers[0]);
                        }
                        DhcpOption::SubnetMask(subnet) => lease.subnet_mask = Some(subnet),
                        DhcpOption::ServerIdentifier(id) => lease.server_id = Some(id),
                        DhcpOption::IpAddressLeaseTime(secs) => lease.lease_duration = secs,
                        _ => {}
                    }
                }
                lease.offer = Some(msg);
                return Ok(lease);
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                println!("WouldBlock Error");
                continue;
            }
            Err(e) => {
                print!("Error, {}", e);
                return Err(e.into());
            }
        }
    }
    Err("Failed after retry.".into())
}
