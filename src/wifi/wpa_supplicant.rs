use std::error::Error;
use std::ffi::CString;
use std::fs::{self, write};
use std::io::{self, Cursor};
use std::mem::MaybeUninit;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixDatagram;
use std::time::{Duration, Instant};
use std::{slice, thread};

use dhcp4r::options::{DhcpOption, MessageType};
use dhcp4r::packet::Packet;
use etherparse::PacketBuilder;
use neli::attr::Attribute;
use neli::consts::nl::NlmF;
use neli::consts::rtnl::{RtAddrFamily, RtScope, RtTable, Rta, Rtm, Rtn, Rtprot};
use neli::consts::socket::{Msg, NlFamily};
use neli::nl::{NlPayload, Nlmsghdr, NlmsghdrBuilder};
use neli::rtnl::{RtattrBuilder, Rtmsg, RtmsgBuilder};
use neli::socket::NlSocket;
use neli::types::RtBuffer;
use neli::utils::Groups;
use neli::{FromBytes, ToBytes};
use socket2::{Domain, Protocol, Socket, Type};

use crate::backend::functions::list_interfaces;
use crate::debug::log_msg;
use crate::types::{DhcpLease, Interface};
use crate::wifi::dhcp_connection::DhcpStorage;
use crate::wifi::helper::{
    add_addr, create_packet_sockaddr, get_current_ip, get_gateway_ip, get_interfaces,
    manage_lease_thread, remove_lease_and_gateway_ip, return_on_disconnect, set_default_route,
    set_iface_up, validate_packet,
};
use crate::{Log, mac_to_bytes};

pub fn connect(iface: &Interface, ssid: &str, password: &str) -> Result<(), Box<dyn Error>> {
    let ifname = iface.ifname.as_ref().ok_or("Interface Name not found.")?;
    let ifindex = iface.ifindex.as_ref().ok_or("Interface Index not found.")?;
    let server_path = format!("/var/run/wpa_supplicant/{}", ifname);
    let client_path = format!("/tmp/beacon_wpa_{}", rand::random::<u32>());

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

    if send_wpa_cmd(&skt, "PING")? != "PONG" {
        return Err("wpa_supplicant did not respond.".into());
    }

    // remove any previous network
    let _ = send_wpa_cmd(&skt, "REMOVE_NETWORK all");

    if send_wpa_cmd(&skt, "ATTACH")? != "OK" {
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
        let network_id = send_wpa_cmd(&skt, "ADD_NETWORK")?;

        let ssid_cmd = format!("SET_NETWORK {} ssid \"{}\"", network_id, ssid);
        let ssid_ok = send_wpa_cmd(&skt, &ssid_cmd)?;
        if ssid_ok != "OK" {
            return Err(format!("failed to set SSID. {}", ssid_ok).into());
        }

        // set psk
        let psk_ok = send_wpa_cmd(
            &skt,
            &format!("SET_NETWORK {} psk \"{}\"", network_id, password),
        )?;
        if psk_ok != "OK" {
            return Err(format!("failed to set password. {}", psk_ok).into());
        }

        send_wpa_cmd(&skt, "SAVE_CONFIG")?;
        network_id
    };

    // disable other networks incase wpa_supplicant connects to any cached network
    let disable_ok = send_wpa_cmd(&skt, "DISABLE_NETWORK all")?;
    if disable_ok != "OK" {
        return Err(format!("Couldn't disable cached networks. {}", disable_ok).into());
    }

    let select_ok = send_wpa_cmd(&skt, &format!("SELECT_NETWORK {}", network_id))?;
    if select_ok != "OK" {
        return Err(format!("Couldn't connect to {}. {}", ssid, select_ok).into());
    }

    log_msg(&format!("Connecting to {}..", ssid), Log::Info);
    let mut recv_buffer = [0u8; 1024 * 4];

    loop {
        skt.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;

        match skt.recv(&mut recv_buffer) {
            Ok(size) => {
                let event = String::from_utf8_lossy(&recv_buffer[..size])
                    .trim()
                    .to_string();
                if event.contains("CTRL-EVENT-CONNECTED") {
                    break;
                } else if event.contains("CTRL-EVENT-AUTH-REJECT") {
                    send_wpa_cmd(&skt, &format!("REMOVE_NETWORK {}", network_id))?;
                    send_wpa_cmd(&skt, "SAVE_CONFIG")?;
                    return Err("Authentication failed. Try Again.".into());
                } else if event.contains("CTRL-EVENT-NETWORK-NOT-FOUND") {
                    return Err("Network not found, Make sure host is in range.".into());
                } else if event.contains("WRONG_KEY") {
                    return Err("Incorrect Password. Please try again.".into());
                }
            }
            Err(_) => return Err("connection timed out after 10 secs.".into()),
        }
    }
    skt.shutdown(std::net::Shutdown::Both)?;
    let _ = fs::remove_file(client_path);

    // Discover packet sent here
    std::thread::sleep(Duration::from_secs(3));
    let host_data = discover_host(iface)?;

    // Request packet sent here
    if let Some(ip_addr) = host_data.ip_addr {
        request_host_wireless(iface, ip_addr, host_data.server_id)?;
    } else {
        return Err("Failed to get ip address from dhcp server.".into());
    }

    let socket = NlSocket::connect(NlFamily::Route, None, Groups::empty())?;

    // tokio::spawn(connection);

    let ip_addr = host_data.ip_addr.ok_or("No IP address from DHCP.")?;
    if let Some(gateway) = host_data.gateway
        && let Err(e) = apply_network_config(&socket, *ifindex, ip_addr, gateway)
    {
        log_msg(&format!("No Gateway IP found: {}", e), Log::Err);
    }
    log_msg("Applied Network Configurations.", Log::Ok);

    set_dns(host_data.dns_servers)?;

    // create a thread to disconnect completely upon server withdrawal
    let ifindex = *ifindex;
    let ifname = ifname.clone();
    tokio::spawn(async move {
        // wait for disconnection

        match return_on_disconnect(ifindex as i32) {
            Ok(_) => {
                log_msg(
                    &format!("Engaging Full Disconnection from {}", ifindex),
                    Log::Info,
                );
                if let Err(e) = disconnect(&ifname, false) {
                    log_msg(&format!("Disconnection ERROR: {}", e), Log::Err);
                };
                // engage complete disconnection
            }
            Err(e) => log_msg(
                &format!("Error while checking for disconnection.\n{}", e),
                Log::Err,
            ),
        };
    });

    /*
     * This is for managing the lease connection in a separate thread
     * i.e: rebinding leaase
     */
    manage_lease_thread(iface)?;
    Ok(())
}

pub fn disconnect(ifname: &str, grace: bool) -> Result<(), Box<dyn Error>> {
    let server_path = format!("/var/run/wpa_supplicant/{}", ifname);
    let client_path = format!("/tmp/beacon_wpa_{}", rand::random::<u32>());
    let _ = fs::remove_file(&client_path);
    let wpa_skt = UnixDatagram::bind(&client_path)?;
    let ifaces = list_interfaces()?;
    let iface = ifaces
        .iter()
        .find(|i| i.ifname == Some(ifname.to_string()))
        .ok_or("Interface not found.")?
        .to_owned();
    let ifindex = iface.ifindex.ok_or("Couldn't parse Ifindex.")?;
    let mac = mac_to_bytes(&iface.mac.ok_or("Couldn't parse mac.")?);

    let ip_addr = get_current_ip(None)?.ok_or("No Current IP found.")?;
    let prefix_len = 32;

    let gateway_ip = get_gateway_ip().ok_or("Gateway IP not found.")?;

    wpa_skt
        .connect(&server_path)
        .map_err(|_| "wpa_supplicant not running or Wifi is turned off.")?;

    if grace {
        let send_socket = UdpSocket::bind("0.0.0.0:0")?;
        send_socket.set_broadcast(true)?;
        bind_socket_to_device(&send_socket, ifname)?;
        let packet = Packet {
            reply: false,
            hops: 0,
            xid: rand::random(),
            ciaddr: ip_addr,
            chaddr: mac,
            secs: 0,
            broadcast: false,
            yiaddr: Ipv4Addr::new(0, 0, 0, 0),
            siaddr: Ipv4Addr::new(0, 0, 0, 0),
            giaddr: Ipv4Addr::new(0, 0, 0, 0),
            options: vec![DhcpOption::DhcpMessageType(MessageType::Release)],
        };
        let dest = gateway_ip.to_string() + ":255";
        let mut buf = [0u8; 1500];
        let data = packet.encode(&mut buf);
        send_socket.send_to(data, dest.clone())?;
        log_msg("Notified Server for Disconnection.", Log::Info);
    }
    if send_wpa_cmd(&wpa_skt, "PING")? != "PONG" {
        return Err("wpa_supplicant did not respond.".into());
    }

    if send_wpa_cmd(&wpa_skt, "ATTACH")? != "OK" {
        return Err("Couldn't connect to wpa_supplicant.".into());
    }

    send_wpa_cmd(&wpa_skt, "DISCONNECT")?;

    let timeout = Duration::from_secs(3);
    let start = Instant::now();
    thread::spawn(move || {
        loop {
            if start.elapsed() >= timeout {
                log_msg("Disconnection Timeout.", Log::Err);
                break;
            }
            if let Err(e) = remove_lease_and_gateway_ip(ifindex, ip_addr, gateway_ip, prefix_len) {
                log_msg(&e.to_string(), Log::Err);
            } else {
                break;
            };
        }
    });
    let _ = fs::remove_file(client_path);
    set_dns(vec![])?;

    Ok(())
}

fn send_wpa_cmd(socket: &UnixDatagram, cmd: &str) -> Result<String, Box<dyn Error>> {
    socket.send(cmd.as_bytes())?;
    let mut buf = [0u8; 1024 * 4];
    while let Ok(size) = socket.recv(&mut buf) {
        let reply = String::from_utf8_lossy(buf[..size].into())
            .trim()
            .to_string();
        if reply.starts_with('<') {
            continue;
        }
        return Ok(reply);
    }
    Ok("FAIL".to_string())
}

pub fn find_active_interface() -> Result<Option<Interface>, Box<dyn Error>> {
    let ifaces = get_interfaces()?;
    let socket = NlSocket::connect(NlFamily::Route, None, Groups::empty())?;

    let mut rtbuf = RtBuffer::new();
    rtbuf.push(
        RtattrBuilder::default()
            .rta_type(Rta::Dst)
            .rta_payload(vec![8, 8, 8, 8])
            .build()?,
    );

    let rtmsg = RtmsgBuilder::default()
        .rtm_family(RtAddrFamily::Inet)
        .rtm_dst_len(32)
        .rtm_src_len(0)
        .rtm_tos(0)
        .rtm_table(RtTable::Main)
        .rtm_protocol(Rtprot::Unspec)
        .rtm_scope(RtScope::Universe)
        .rtm_type(Rtn::Unicast)
        .rtattrs(rtbuf)
        .build()?;

    let nl_msg = NlmsghdrBuilder::default()
        .nl_flags(NlmF::REQUEST)
        .nl_type(Rtm::Getroute)
        .nl_payload(NlPayload::Payload(rtmsg))
        .build()?;

    let mut msg_buf = Cursor::new(Vec::<u8>::new());
    nl_msg.to_bytes(&mut msg_buf)?;

    socket.send(msg_buf.get_ref(), Msg::empty())?;

    let mut recv_buf = [0u8; 4096 * 16];
    let (size, _) = socket.recv(&mut recv_buf, Msg::empty())?;
    let mut res_buf = Cursor::new(&recv_buf[..size]);
    let res: Nlmsghdr<Rtm, Rtmsg> = Nlmsghdr::from_bytes(&mut res_buf)?;

    if let NlPayload::Err(e) = res.nl_payload() {
        return Err(format!("Kernel Error: {}", e).into());
    }

    let mut ifindex: Option<u32> = None;
    if let NlPayload::Payload(link_info) = res.nl_payload() {
        for attr in link_info.rtattrs().iter() {
            if *attr.rta_type() == Rta::Oif {
                ifindex = Some(attr.get_payload_as::<u32>()?);
                break;
            }
        }
    }
    let result = ifaces
        .iter()
        .find(|iface| iface.ifindex == ifindex)
        .cloned();
    Ok(result)
}

fn apply_network_config(
    socket: &NlSocket,
    ifindex: u32,
    ip: Ipv4Addr,
    gateway: Ipv4Addr,
) -> Result<(), Box<dyn Error>> {
    add_addr(socket, ifindex, ip)?;
    set_default_route(socket, ifindex, gateway)?;
    Ok(())
}

fn set_dns(dns_servers: Vec<Ipv4Addr>) -> Result<(), Box<dyn Error>> {
    let mut config_lines = Vec::<String>::new();
    for dns in dns_servers {
        config_lines.push(format!("nameserver {}", dns));
    }
    // fallback DNS's
    config_lines.push("nameserver 8.8.8.8".to_string());
    config_lines.push("nameserver 1.1.1.1".to_string());
    write("/etc/resolv.conf", config_lines.join("\n"))?;
    Ok(())
}

pub fn request_host_wireless(
    iface: &Interface,
    current_ip: Ipv4Addr,
    server_id: Option<Ipv4Addr>,
) -> Result<DhcpLease, Box<dyn Error>> {
    let mut options = Vec::new();

    // message type requesst
    options.push(DhcpOption::DhcpMessageType(MessageType::Request));

    // requested ip address (optins 50)
    options.push(DhcpOption::RequestedIpAddress(current_ip));

    if let Some(server_id) = server_id {
        options.push(DhcpOption::ServerIdentifier(server_id));
    }

    // ask for same things as before
    options.push(DhcpOption::ParameterRequestList(vec![1, 3, 6, 15, 51]));

    let xid = rand::random();
    let mac = iface.mac.as_ref().ok_or("No MAC for interface.")?;
    let ifindex = iface.ifindex.as_ref().ok_or("No ifindex.")?;
    let ifname = iface.ifname.as_ref().ok_or("No ifname.")?;
    let request_packet = Packet {
        reply: false,
        hops: 0,
        xid,
        ciaddr: current_ip,
        chaddr: mac_to_bytes(mac),
        secs: 0,
        broadcast: true,
        yiaddr: Ipv4Addr::new(0, 0, 0, 0),
        siaddr: Ipv4Addr::new(0, 0, 0, 0),
        giaddr: Ipv4Addr::new(0, 0, 0, 0),
        options,
    };

    // bind socket to 0.0.0.0 because we dont yet have an IP
    let send_socket = UdpSocket::bind("0.0.0.0:68")?;
    let socket = Socket::new(
        Domain::PACKET,
        Type::RAW,
        Some(Protocol::from(libc::ETH_P_ALL)),
    )?;

    // allow broadcasting
    send_socket.set_broadcast(true)?;

    bind_socket_to_device(&send_socket, ifname)?;
    socket.bind_device(Some(ifname.as_bytes()))?;
    // let sockaddr = SockAddr::from(SocketAddrV4::new(current_ip, 67));
    let sockaddr = create_packet_sockaddr(*ifindex);
    socket.bind(&sockaddr)?;

    let dest = "255.255.255.255:67";

    let mut buf = [0u8; 1500];
    let data = request_packet.encode(&mut buf);

    send_socket.send_to(data, dest)?;

    let timeout = Instant::now();
    let mut buf = [MaybeUninit::<u8>::zeroed(); 1500];
    let mut result = DhcpLease::default();
    loop {
        if timeout.elapsed() >= Duration::from_secs(5) {
            log_msg(&format!("{ifname} connection Timeout"), Log::Warn);
            break;
        }
        match socket.recv(&mut buf) {
            Ok(size) => {
                let raw_data = unsafe { slice::from_raw_parts(buf.as_ptr() as *const u8, size) };
                let packet = match validate_packet(raw_data, size)? {
                    Some(s) => s,
                    None => {
                        continue;
                    }
                };
                if packet.xid != request_packet.xid {
                    continue;
                }
                let mut is_ack = false;
                for option in packet.options {
                    match option {
                        DhcpOption::DhcpMessageType(val) => match val {
                            MessageType::Ack => {
                                log_msg("Server Acknowledged Wireless", Log::Ok);
                                is_ack = true;
                                result.ip_addr = if packet.yiaddr.is_unspecified() {
                                    Some(current_ip)
                                } else {
                                    Some(packet.yiaddr)
                                };
                            }
                            MessageType::Nak => {
                                log_msg("Server Refused to Acknowledge Wireless", Log::Err);
                                break;
                            }
                            _ => {}
                        },
                        DhcpOption::DomainNameServer(ips) => result.dns_servers = ips,
                        DhcpOption::Router(routers) if !routers.is_empty() => {
                            result.gateway = Some(routers[0]);
                        }
                        DhcpOption::SubnetMask(subnet) => result.subnet_mask = Some(subnet),
                        DhcpOption::ServerIdentifier(id) => result.server_id = Some(id),
                        DhcpOption::IpAddressLeaseTime(secs) => result.lease_duration = secs,
                        _ => {}
                    }
                }
                if is_ack {
                    result.offer = Some(request_packet);
                    DhcpStorage::write_from_dhcplease(&result, ifname.clone())?;
                    return Ok(result);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                log_msg("Connection already exists, proceeding safely..", Log::Info);
                break;
            }
            Err(e) => {
                log_msg(&format!("Error while Connecting Wireless: {}", e), Log::Err);
                return Err(e.into());
            }
        }
    }
    Err("Failed to receive DHCP ACK.".into())
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

pub fn discover_host(iface: &Interface) -> Result<DhcpLease, Box<dyn Error>> {
    let ifname = iface.ifname.as_ref().ok_or("No ifname.")?;
    let ifindex = iface.ifindex.as_ref().ok_or("No ifindex.")?;
    let mac = iface.mac.as_ref().ok_or("No MAC.")?;

    let mac = mac_to_bytes(mac);
    // point it at global broadcast address
    // socket used only for sending signals
    // let broadcast_addr: SockAddr =
    //     SockAddr::from(SocketAddrV4::new(Ipv4Addr::new(255, 255, 255, 255), 68));
    let total_retries = 5;
    let xid = rand::random();

    let socket = socket2::Socket::new(
        Domain::PACKET,
        Type::RAW,
        Some(Protocol::from((libc::ETH_P_ALL as u16).to_be() as i32)),
    )?;
    socket.bind_device(Some(ifname.as_bytes()))?;
    let nlsock = NlSocket::connect(NlFamily::Route, None, Groups::empty())?;

    set_iface_up(&nlsock, *ifindex as i32)?;
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
            chaddr: mac,
            options: vec![
                DhcpOption::DhcpMessageType(MessageType::Discover),
                DhcpOption::ParameterRequestList(vec![2, 3, 6, 15, 51]), // Subnet, Router, DNS, Domain
            ],
        };

        let mut buf = [0u8; 1500];
        let slice = msg.encode(&mut buf);

        let ethheader = PacketBuilder::ethernet2(mac, [255, 255, 255, 255, 255, 255])
            .ipv4([0, 0, 0, 0], [255, 255, 255, 255], 64)
            .udp(68, 67);
        let mut full_packet = Vec::<u8>::with_capacity(ethheader.size(slice.len()));
        ethheader.write(&mut full_packet, slice)?;

        let sockaddr = create_packet_sockaddr(*ifindex);
        // sending the socket
        socket.send_to(&full_packet, &sockaddr)?;

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
                log_msg(&format!("{ifname} discovery Timeout."), Log::Warn);
                break;
            }
            match socket.recv_from(&mut res_buf) {
                Ok((size, _)) => {
                    let initialized_data =
                        unsafe { std::slice::from_raw_parts(res_buf.as_ptr() as *const u8, size) };

                    let packet = match validate_packet(initialized_data, size)? {
                        Some(s) => s,
                        None => {
                            continue;
                        }
                    };

                    if packet.xid != msg.xid {
                        continue;
                    }

                    for option in packet.options {
                        match option {
                            DhcpOption::DhcpMessageType(val) => match val {
                                dhcp4r::options::MessageType::Offer => {
                                    ip_addr = Some(packet.yiaddr);
                                }
                                _ => continue,
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
                    if ip_addr.is_some() {
                        let result = DhcpLease {
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
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }
    }
    Err("Failed after retry.".into())
}

pub fn request_host_wired(
    mac_address: [u8; 6],
    current_ip: Ipv4Addr,
    server_id: Ipv4Addr,
    iface: &Interface,
    broadcast: bool,
) -> Result<DhcpLease, Box<dyn Error>> {
    let iface = iface.clone();
    let ifname = iface.ifname.unwrap_or_default();
    let ifindex = iface.ifindex.unwrap_or_default();
    let socket = socket2::Socket::new(
        Domain::PACKET,
        Type::RAW,
        Some(Protocol::from(libc::ETH_P_ALL)),
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

    let sockaddr = create_packet_sockaddr(ifindex);
    socket.bind(&sockaddr)?;
    send_socket.set_broadcast(true)?;

    bind_socket_to_device(&send_socket, &ifname)?;
    socket.bind_device(Some(ifname.as_bytes()))?;

    let timeout = Instant::now();

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
    let mut lease = DhcpLease::default();
    let mut res_buf = [MaybeUninit::<u8>::zeroed(); 1500];

    loop {
        if timeout.elapsed() >= Duration::from_secs(5) {
            log_msg(&format!("{ifname} connection Timeout."), Log::Warn);
            break;
        }
        match socket.recv(&mut res_buf) {
            Ok(size) => {
                let raw_data =
                    unsafe { std::slice::from_raw_parts(res_buf.as_ptr() as *const u8, size) };
                let packet = match validate_packet(raw_data, size)? {
                    Some(s) => s,
                    None => {
                        continue;
                    }
                };
                if packet.xid != msg.xid {
                    continue;
                }
                let mut is_ack = false;
                for option in packet.options {
                    match option {
                        DhcpOption::DhcpMessageType(val) => match val {
                            MessageType::Ack => {
                                log_msg("Server Acknowledged Wired", Log::Ok);
                                is_ack = true;
                                lease.ip_addr = if packet.yiaddr.is_unspecified() {
                                    Some(current_ip)
                                } else {
                                    Some(packet.yiaddr)
                                };
                            }
                            MessageType::Nak => {
                                log_msg("Server Refused to Acknowledge Wired", Log::Err);
                                break;
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
                if is_ack {
                    lease.offer = Some(msg);
                    DhcpStorage::write_from_dhcplease(&lease, ifname)?;
                    return Ok(lease);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => {
                log_msg(&format!("Error in connecting Wired: {}", e), Log::Err);
                return Err(e.into());
            }
        }
    }
    Err("Failed to receive DHCP ACK.".into())
}

pub fn connect_via_ethernet(iface: &Interface) -> Result<(), Box<dyn Error>> {
    // setting up USB ethernet
    let socket = NlSocket::connect(NlFamily::Route, None, Groups::empty())?;
    let ifindex = iface.ifindex.as_ref().ok_or("No ifindex.")?;
    let ifname = iface.ifname.as_ref().ok_or("No ifname.")?;
    set_iface_up(&socket, *ifindex as i32)?;

    let data = discover_host(iface)?;

    if let Some(offer) = data.offer
        && let Some(server_id) = data.gateway
        && let Some(current_ip) = data.ip_addr
    {
        let mac_address = offer.chaddr;

        let edata = request_host_wired(mac_address, current_ip, server_id, iface, true)?;
        DhcpStorage::write_from_dhcplease(&edata, ifname.to_string())?;
        add_addr(&socket, *ifindex, current_ip)?;
        set_default_route(&socket, *ifindex, server_id)?;
        set_dns(edata.dns_servers)?;
        Ok(())
    } else {
        Err("Fields missing! [wpa_supplicant]".into())
    }
}
