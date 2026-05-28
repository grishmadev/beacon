use crate::backend::functions::connect_to;
use crate::mac_to_bytes;
use crate::types::{CurrentConnection, DhcpLease, FamilyInfo, Host, Interface, InterfaceType};
use crate::wifi::dhcp_connection::DhcpStorage;
use crate::wifi::history::list_saved_networks;
use crate::wifi::wpa_supplicant::{
    find_active_interface, request_host_wired, request_host_wireless,
};
use chrono::Utc;
use dhcp4r::packet::Packet;
use libc::RTMGRP_LINK;
use neli::consts::rtnl::{Ifa, IfaF, RtTable, Rta, RtmF, Rtn, Rtprot};
use neli::err::Nlmsgerr;
use neli::rtnl::{Ifaddrmsg, IfaddrmsgBuilder, RtattrBuilder, RtmsgBuilder};
use neli::socket::synchronous::NlSocketHandle;
use neli::types::RtBuffer;
use neli::{
    FromBytes, ToBytes,
    attr::Attribute,
    consts::{
        genl::{CtrlAttr, CtrlAttrMcastGrp, CtrlCmd},
        nl::{GenlId, NlmF},
        rtnl::{Arphrd, Iff, Ifla, RtAddrFamily, RtScope, Rtm},
        socket::{Msg, NlFamily},
    },
    genl::{AttrType, AttrTypeBuilder, Genlmsghdr, GenlmsghdrBuilder, Nlattr, NlattrBuilder},
    nl::{NlPayload, Nlmsghdr, NlmsghdrBuilder},
    rtnl::{Ifinfomsg, IfinfomsgBuilder},
    socket::NlSocket,
    types::{Buffer, GenlBuffer},
    utils::Groups,
};
use socket2::SockAddr;
use std::fs::{self, File};
use std::io::{self, Read};
use std::net::Ipv4Addr;
use std::time::Duration;
use std::{error::Error, io::Cursor, path::Path};

use nl80211::{Nl80211Attr, Nl80211Bss, Nl80211Cmd};

pub fn get_family_info() -> Result<FamilyInfo, Box<dyn Error + Send + Sync>> {
    let sock = NlSocket::new(NlFamily::Generic)?;
    let mut family_name = b"nl80211".to_vec();
    family_name.push(0);

    let family_name_attr_type = AttrTypeBuilder::default()
        .nla_type(CtrlAttr::FamilyName)
        .build()?;

    let name_attribute = NlattrBuilder::default()
        .nla_type(family_name_attr_type)
        .nla_payload(family_name)
        .build()?;

    // create a buffer to store attribute
    let mut attr_buffer: GenlBuffer<CtrlAttr, neli::types::Buffer> = GenlBuffer::new();
    attr_buffer.push(name_attribute);

    let genl_header = GenlmsghdrBuilder::<CtrlCmd, CtrlAttr>::default()
        .cmd(CtrlCmd::Getfamily)
        .version(1)
        .attrs(attr_buffer)
        .build()?;

    let nl_msg = NlmsghdrBuilder::default()
        .nl_flags(NlmF::REQUEST | NlmF::ACK)
        .nl_type(GenlId::Ctrl)
        .nl_payload(NlPayload::Payload(genl_header))
        .build()?;

    let mut msg_buffer = std::io::Cursor::new(Vec::<u8>::new());
    nl_msg.to_bytes(&mut msg_buffer)?;

    sock.send(msg_buffer.get_ref(), Msg::empty())?;

    let mut recv_buffer = [0u8; 4096];

    let (size, _) = sock.recv(&mut recv_buffer, Msg::empty())?;

    let mut cursor = std::io::Cursor::new(&recv_buffer[..size]);

    let res: Nlmsghdr<GenlId, Genlmsghdr<CtrlCmd, CtrlAttr>> = Nlmsghdr::from_bytes(&mut cursor)?;

    if let NlPayload::Err(e) = res.nl_payload() {
        return Err(format!("Kernel Error: {}", e).into());
    }

    let mut family_info = FamilyInfo::default();
    let mut group_name = String::new();
    let mut group_id = 0u32;
    if let NlPayload::Payload(genl) = res.nl_payload() {
        let attrs = genl.attrs();
        for attr in attrs.iter() {
            if *attr.nla_type().nla_type() == CtrlAttr::FamilyId {
                let id: u16 = attr.get_payload_as()?;
                family_info.id = id;
            }
            if *attr.nla_type().nla_type() == CtrlAttr::FamilyName {
                let payload = attr.nla_payload().as_ref();
                let name = String::from_utf8_lossy(payload)
                    .trim_end_matches('\0')
                    .to_string();
                family_info.name = name;
            }
            if *attr.nla_type().nla_type() == CtrlAttr::McastGroups {
                let payload = attr.nla_payload().as_ref();
                let mut outer_cursor = Cursor::new(payload);
                'outer: while outer_cursor.position() < payload.len() as u64 {
                    if let Ok(group) = Nlattr::<u16, Buffer>::from_bytes(&mut outer_cursor) {
                        let group_bytes = group.nla_payload().as_ref();
                        let mut inner_cursor = Cursor::new(group_bytes);
                        while inner_cursor.position() < group_bytes.len() as u64 {
                            if let Ok(inner) = Nlattr::<u16, Buffer>::from_bytes(&mut inner_cursor)
                            {
                                let inner_payload = inner.nla_payload().as_ref();
                                let inner_type = *inner.nla_type().nla_type();
                                match CtrlAttrMcastGrp::from(inner_type) {
                                    CtrlAttrMcastGrp::Name => {
                                        group_name = String::from_utf8_lossy(inner_payload)
                                            .trim_end_matches('\0')
                                            .to_string();
                                    }
                                    CtrlAttrMcastGrp::Id => {
                                        let id = u32::from_le_bytes(inner_payload[..4].try_into()?);
                                        group_id = id;
                                    }
                                    _ => {}
                                }
                            } else {
                                break;
                            }
                        }
                        if group_name == "scan" {
                            family_info.scan_group_id = group_id;
                            break 'outer; // found what we needed.
                        }
                    } else {
                        break;
                    }
                }
            }
        }
    }
    Ok(family_info)
}

pub fn get_scan(family_id: u16, ifindex: u32) -> Result<Vec<Host>, Box<dyn Error + Send + Sync>> {
    let mut result = Vec::<Host>::new();
    let sock = NlSocket::new(NlFamily::Generic)?;

    // Build from ifindex attribute
    let attr_type: AttrType<u16> = AttrTypeBuilder::default()
        .nla_type(Nl80211Attr::AttrIfindex.into())
        .build()?;

    let ifindex_attr = NlattrBuilder::default()
        .nla_type(attr_type)
        .nla_payload(ifindex.to_ne_bytes().to_vec())
        .build()?;

    // Send GETSCAN with DUMP flag (return all Access POINT)
    let mut attr_buffer: GenlBuffer<u16, Buffer> = GenlBuffer::new();
    attr_buffer.push(ifindex_attr);
    let genl_header = GenlmsghdrBuilder::<u8, u16>::default()
        .cmd(Nl80211Cmd::CmdGetScan.into())
        .version(1)
        .attrs(attr_buffer)
        .build()?;
    let nl_msg = NlmsghdrBuilder::default()
        .nl_flags(NlmF::REQUEST | NlmF::DUMP)
        .nl_type(family_id)
        .nl_payload(NlPayload::Payload(genl_header))
        .build()?;

    let mut msg_buffer = Cursor::new(Vec::<u8>::new());

    nl_msg.to_bytes(&mut msg_buffer)?;
    sock.send(msg_buffer.get_ref(), Msg::empty())?;

    // parse received Buffer

    let mut recv_buffer = [0u8; 4096 * 16];

    loop {
        let (size, _) = sock.recv(&mut recv_buffer, Msg::empty())?;
        let mut cursor = Cursor::new(&recv_buffer[..size]);
        while cursor.position() < size as u64 {
            let res: Nlmsghdr<u16, Genlmsghdr<u8, u16>> = Nlmsghdr::from_bytes(&mut cursor)?;

            if let NlPayload::Err(e) = res.nl_payload() {
                return Err(format!("Kernel Error: {}", e).into());
            }

            if *res.nl_type() == libc::NLMSG_DONE as u16 {
                return Ok(result);
            }

            if let NlPayload::Payload(genl) = res.nl_payload() {
                let attrs = genl.attrs();

                for attr in attrs.iter() {
                    let typ = Nl80211Attr::from(*attr.nla_type().nla_type());
                    if typ == Nl80211Attr::AttrBss {
                        let bss_bytes = attr.nla_payload().as_ref();

                        let mut bss_cursor = Cursor::new(bss_bytes);
                        let mut target = Host::new();

                        while let Ok(nested) = Nlattr::<u16, Buffer>::from_bytes(&mut bss_cursor) {
                            match Nl80211Bss::from(*nested.nla_type().nla_type()) {
                                Nl80211Bss::BssBssid => {
                                    let bytes = nested.nla_payload().as_ref();
                                    if bytes.len() >= 6 {
                                        let mac = bytes[..6]
                                            .iter()
                                            .map(|b| format!("{b:02X}"))
                                            .collect::<Vec<_>>()
                                            .join(":");
                                        target.set_bssid(mac);
                                    }
                                }

                                Nl80211Bss::BssFrequency => {
                                    let bytes = nested.nla_payload().as_ref();
                                    if bytes.len() >= 4 {
                                        let freq = u32::from_le_bytes(bytes[..4].try_into()?);
                                        target.set_frequency(freq);
                                    }
                                }

                                Nl80211Bss::BssSignalMbm => {
                                    let bytes = nested.nla_payload().as_ref();
                                    if bytes.len() >= 4 {
                                        let signal = i32::from_le_bytes(bytes[..4].try_into()?) / 100;
                                        target.set_signal(signal);
                                    }
                                }

                                Nl80211Bss::BssInformationElements => {
                                    let ies = nested.nla_payload().as_ref();
                                    let mut i = 0;
                                    while i + 1 < ies.len() {
                                        let tag = ies[i];
                                        let len = ies[i + 1] as usize;
                                        if i + 2 + len > ies.len() {
                                            break;
                                        }
                                        if tag == 0 {
                                            let ssid =
                                                String::from_utf8_lossy(&ies[i + 2..i + 2 + len])
                                                    .to_string();
                                            target.set_ssid(ssid);
                                        }
                                        i += 2 + len;
                                    }
                                }

                                Nl80211Bss::BssStatus => {
                                    let payload = nested.nla_payload().as_ref();
                                    if payload.len() >= 4 {
                                        let status = u32::from_le_bytes(payload[..4].try_into()?);
                                        target.is_connected = status == 1;
                                    }
                                }

                                _ => {}
                            }
                        }
                        result.push(target);
                    }
                }
            }
        }
    }
}

pub fn get_interfaces() -> Result<Vec<Interface>, Box<dyn Error>> {
    let sock = NlSocket::new(NlFamily::Route)?;
    // attribute not needed in requ\t
    // DUMP flag asks for all AP

    let ifinfo = IfinfomsgBuilder::default()
        .ifi_family(RtAddrFamily::Unspecified)
        .ifi_type(Arphrd::None)
        .ifi_index(0)
        .ifi_change(Iff::empty())
        .ifi_flags(Iff::empty())
        .build()?;

    let nl_msg = NlmsghdrBuilder::default()
        .nl_flags(NlmF::DUMP | NlmF::REQUEST)
        .nl_type(Rtm::Getlink)
        .nl_payload(NlPayload::Payload(ifinfo))
        .build()?;

    let mut msg_buffer = Cursor::new(Vec::<u8>::new());
    nl_msg.to_bytes(&mut msg_buffer)?;
    sock.send(msg_buffer.get_ref(), Msg::empty())?;
    let mut result = Vec::<Interface>::new();
    loop {
        let mut recv_buffer = [0u8; 4096 * 16];
        let (size, _) = sock.recv(&mut recv_buffer, Msg::empty())?;
        let mut cursor = Cursor::new(&recv_buffer[..size]);

        while cursor.position() < size as u64 {
            let res: Nlmsghdr<Rtm, Ifinfomsg> = Nlmsghdr::from_bytes(&mut cursor)?;

            if let NlPayload::Err(e) = res.nl_payload() {
                return Err(format!("Kernel Error: {}", e).into());
            }

            if u16::from(*res.nl_type()) == libc::NLMSG_DONE as u16 {
                return Ok(result);
            }

            if let NlPayload::Payload(link_info) = res.nl_payload() {
                let mut iface = Interface::new();
                iface.set_ifindex(*link_info.ifi_index() as u32);
                for attr in link_info.rtattrs().iter() {
                    match attr.rta_type() {
                        Ifla::Ifname => {
                            let name = attr.get_payload_as_with_len::<String>()?;
                            iface.set_ifname(name.to_string());
                            if name.starts_with("wl") || is_wireless(&name) {
                                iface.set_iftype(InterfaceType::Wireless);
                            } else if name.starts_with("eth") || name.starts_with("en") {
                                iface.set_iftype(InterfaceType::Wired);
                            } else if name.contains("lo") {
                                iface.set_iftype(InterfaceType::Loopback);
                            }
                        }
                        Ifla::Address => {
                            let payload = attr.rta_payload().as_ref();
                            if payload.len() == 6 {
                                let mac = payload
                                    .iter()
                                    .map(|b| format!("{b:02X}"))
                                    .collect::<Vec<String>>()
                                    .join(":");
                                iface.set_mac(mac);
                            }
                        }
                        Ifla::Operstate => {
                            // let payload = attr.rta_payload().as_ref();
                            // println!("operstate: {:#?}", payload);
                        }
                        _ => {}
                    }
                }
                result.push(iface);
            }
        }
    }
}

fn is_wireless(ifname: &str) -> bool {
    let ifpath = format!("/sys/class/net/{}/wireless", ifname);
    Path::new(&ifpath).exists()
}

pub fn get_current(family_id: u16) -> Result<Option<CurrentConnection>, Box<dyn Error>> {
    let sock = NlSocket::new(NlFamily::Generic)?;
    let attr_buffer: GenlBuffer<u16, Buffer> = GenlBuffer::new();
    let genl_header = GenlmsghdrBuilder::<u8, u16>::default()
        .cmd(Nl80211Cmd::CmdGetInterface.into())
        .version(1)
        .attrs(attr_buffer)
        .build()?;

    let nl_msg = NlmsghdrBuilder::default()
        .nl_flags(NlmF::REQUEST | NlmF::DUMP)
        .nl_type(family_id)
        .nl_payload(NlPayload::Payload(genl_header))
        .build()?;
    let mut msg_buffer = Cursor::new(Vec::<u8>::new());
    nl_msg.to_bytes(&mut msg_buffer)?;
    sock.send(msg_buffer.get_ref(), Msg::empty())?;
    loop {
        let mut recv_buffer = [0u8; 1024 * 64];
        let (size, _) = sock.recv(&mut recv_buffer, Msg::empty())?;
        let mut cursor = Cursor::new(&recv_buffer[..size]);
        while cursor.position() < size as u64 {
            let res: Nlmsghdr<u16, Genlmsghdr<u8, u16>> = Nlmsghdr::from_bytes(&mut cursor)?;

            if let NlPayload::Err(e) = res.nl_payload() {
                return Err(format!("Kernel Error: {}", e).into());
            }

            if *res.nl_type() == libc::NLMSG_DONE as u16 {
                return Ok(None);
            }

            if let NlPayload::Payload(genl) = res.nl_payload() {
                let mut connection = CurrentConnection::new();
                for attr in genl.attrs().iter() {
                    let payload = attr.nla_payload().as_ref();
                    match Nl80211Attr::from(*attr.nla_type().nla_type()) {
                        Nl80211Attr::AttrIfname => {
                            let name = String::from_utf8_lossy(payload)
                                .trim_end_matches('\0')
                                .to_string();
                            connection.ifname = Some(name);
                        }

                        Nl80211Attr::AttrMac if payload.len() >= 6 => {
                            let mac = payload[..6]
                                .iter()
                                .map(|b| format!("{b:02X}"))
                                .collect::<Vec<_>>()
                                .join(":");
                            connection.mac = Some(mac);
                        }
                        Nl80211Attr::AttrIfindex if payload.len() >= 4 => {
                            let ifindex = u32::from_le_bytes(payload[..4].try_into()?);
                            connection.ifindex = Some(ifindex);
                        }
                        Nl80211Attr::AttrSsid => {
                            let _ssid = String::from_utf8_lossy(payload).to_string();
                        }

                        _ => {}
                    }
                }
                if let Some(ifindex) = connection.ifindex {
                    let hosts = get_scan(family_id, ifindex).unwrap_or_default();
                    match hosts.into_iter().find(|h| h.is_connected) {
                        Some(host) => {
                            connection.ssid = host.ssid;
                            connection.bssid = host.bssid;
                            connection.frequency = host.frequency;
                        }
                        None => return Ok(None),
                    };
                }
                connection.ip_addr = get_current_ip(connection.ifindex).ok().flatten();
                connection.gateway = get_gateway_ip();
                if let Ok(files) = DhcpStorage::read_file()
                    && let Some(edata) = files.first()
                {
                    connection.subnet_mask = edata.subnet_mask;
                    connection.dns_servers = edata.dns_servers.to_owned();
                    connection.server_id = edata.server_id;
                    connection.lease_duration = edata.lease_duration;
                    connection.time_initiated = edata.time_initiated;
                }
                return Ok(Some(connection));
            }
        }
    }
}

pub fn get_current_ip(ifindex: Option<u32>) -> Result<Option<Ipv4Addr>, Box<dyn Error>> {
    let ifindex = match ifindex {
        Some(idx) => idx,
        None => {
            let active_iface = find_active_interface()?.ok_or("Cannot find Active Interface")?;
            active_iface.ifindex.expect("No Index found.")
        }
    };
    let socket = NlSocketHandle::connect(NlFamily::Route, None, Groups::empty())?;
    let ifaddrmsg = IfaddrmsgBuilder::default()
        .ifa_family(RtAddrFamily::Inet)
        .ifa_prefixlen(0)
        .ifa_scope(RtScope::Universe)
        .ifa_index(ifindex)
        .build()?;

    let nlhdr: Nlmsghdr<Rtm, Ifaddrmsg> = NlmsghdrBuilder::default()
        .nl_flags(NlmF::DUMP | NlmF::REQUEST)
        .nl_type(Rtm::Getaddr)
        .nl_payload(NlPayload::Payload(ifaddrmsg))
        .build()?;

    socket.send(&nlhdr)?;

    let mut iter = socket.recv::<Rtm, Ifaddrmsg>()?;
    while let Some(Ok(res)) = iter.0.next() {
        match res.nl_payload() {
            NlPayload::Err(e) => return Err(format!("Kernel Error: {}", e).into()),
            NlPayload::Payload(payload) if payload.ifa_index() == &ifindex => {
                for rta in payload.rtattrs().iter() {
                    match rta.rta_type() {
                        Ifa::Local | Ifa::Address => {
                            let bytes = rta.rta_payload().as_ref();

                            if bytes.len() == 4 {
                                let ip = Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]);
                                return Ok(Some(ip));
                            }
                        }
                        _ => continue,
                    }
                }
            }
            _ => {}
        }
    }
    Ok(None)
}

pub fn trigger_scan(family_info: &FamilyInfo, ifindex: u32) -> Result<(), Box<dyn Error>> {
    let sock = NlSocket::new(NlFamily::Generic)?;
    // join scan multicast groups
    sock.add_mcast_membership(Groups::new_groups(&[family_info.scan_group_id]))?;

    // build ifindex attribute
    let attr_type: AttrType<u16> = AttrTypeBuilder::default()
        .nla_type(Nl80211Attr::AttrIfindex.into())
        .build()?;
    let ifindex_attr = NlattrBuilder::default()
        .nla_type(attr_type)
        .nla_payload(ifindex.to_ne_bytes().to_vec())
        .build()?;

    let mut genl_buffer = GenlBuffer::new();
    genl_buffer.push(ifindex_attr);

    let genl_header: Genlmsghdr<u8, u16> = GenlmsghdrBuilder::default()
        .cmd(Nl80211Cmd::CmdTriggerScan.into())
        .version(1)
        .attrs(genl_buffer)
        .build()?;

    let nl_msg = NlmsghdrBuilder::default()
        .nl_flags(NlmF::REQUEST | NlmF::ACK)
        .nl_type(family_info.id)
        .nl_payload(NlPayload::Payload(genl_header))
        .build()?;

    let mut msg_buffer = Cursor::new(Vec::<u8>::new());
    nl_msg.to_bytes(&mut msg_buffer)?;

    // sending msg to socket
    sock.send(msg_buffer.get_ref(), Msg::empty())?;

    let mut recv_buffer = [0u8; 1024 * 64];
    loop {
        let (size, _) = sock.recv(&mut recv_buffer, Msg::empty())?;
        // recieving scans can take 1 - 3 secs

        let mut cursor = Cursor::new(&recv_buffer[..size]);
        let res: Nlmsghdr<u16, Genlmsghdr<u8, u16>> = Nlmsghdr::from_bytes(&mut cursor)?;

        if let NlPayload::Err(e) = res.nl_payload() {
            if e.error() == &-16 {
                // Resource Busy
                print!("Retrying, {}", e);
                continue;
            }
            return Err(format!("Error from trigger_scan: {}", e).into());
        }

        if let NlPayload::Payload(genl) = res.nl_payload() {
            match Nl80211Cmd::from(*genl.cmd()) {
                Nl80211Cmd::CmdNewScanResults => {
                    // scanning finished, new results in cache
                    break;
                }
                Nl80211Cmd::CmdScanAborted => {
                    // Some other process interupted the scan
                    return Err("Scan Aborted.".into());
                }
                _ => {}
            }
        }
    }

    Ok(())
}

pub fn renew_connection(
    iface: &Interface,
    broadcast: bool,
) -> Result<Option<DhcpLease>, Box<dyn Error>> {
    let wired = iface.iftype == InterfaceType::Wired;
    let family_info = get_family_info().unwrap_or_default();
    let family_id = family_info.id;
    let current = get_current(family_id)?.expect("Cannot find any current Connnection :(");

    // IP for this client (This Device)
    let current_ip = current.ip_addr.expect("No IP Address found.");
    let mac = current.mac.expect("No MAC Address found.");
    let mac_address = mac_to_bytes(&mac);

    // IP of the server
    let server_id = current.server_id.expect("NO Server ID found.");

    let data = if wired {
        request_host_wired(mac_address, current_ip, server_id, broadcast)?
    } else {
        request_host_wireless(iface, current_ip, None)?
    };
    Ok(Some(data))
}

pub fn validate_packet(
    initialized_data: &[u8],
    size: usize,
) -> Result<Option<Packet>, Box<dyn Error>> {
    if size < 42 {
        return Ok(None);
    }
    // Protocol Check
    if initialized_data[23] != 17 {
        return Ok(None);
    }
    // Dynamic IP Header Length
    // The lower 4 bits of the first IP byte (at index 14) is the IHL.
    // It represents the number of 32-bit words.
    let ihl = (initialized_data[14] & 0x0F) as usize * 4;
    let udp_start = 14 + ihl;
    let dhcp_start = udp_start + 8;

    if size < dhcp_start {
        return Ok(None);
    }

    let dest_port = u16::from_be_bytes([
        initialized_data[udp_start + 2],
        initialized_data[udp_start + 3],
    ]);
    if dest_port != 68 {
        return Ok(None);
    }
    let dhcp_data = &initialized_data[dhcp_start..size];
    let packet = Packet::from(dhcp_data).map_err(|_| "Failed to parse DHCP Packet.")?;
    Ok(Some(packet))
}

pub fn get_gateway_ip() -> Option<Ipv4Addr> {
    let content = fs::read_to_string("/proc/net/route").ok()?;
    for line in content.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        // Destination 00000000 means the default route
        if fields.get(1)? == &"00000000" {
            let gw_hex = fields.get(2)?;
            let gw_u32 = u32::from_str_radix(gw_hex, 16).ok()?;
            // IPs in /proc are stored in Little Endian hex
            return Some(Ipv4Addr::from(u32::from_be(gw_u32)));
        }
    }
    None
}

pub fn create_packet_sockaddr(ifindex: u32) -> SockAddr {
    unsafe {
        let mut ll: libc::sockaddr_ll = std::mem::zeroed();
        ll.sll_family = libc::AF_PACKET as u16;
        ll.sll_ifindex = ifindex as i32;
        ll.sll_protocol = (libc::ETH_P_ALL as u16).to_be();
        let ptr = &ll as *const libc::sockaddr_ll as *const socket2::SockAddrStorage;
        let storage = std::ptr::read(ptr);

        socket2::SockAddr::new(
            storage,
            std::mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t,
        )
    }
}

pub fn generate_client_id(mac: [u8; 6]) -> Vec<u8> {
    let mut id = Vec::with_capacity(10);
    // DUID Type 3 (Link-layer address)
    id.extend_from_slice(&[0x00, 0x03]);
    // Hardware type: Ethernet (1)
    id.extend_from_slice(&[0x00, 0x01]);
    // The MAC address
    id.extend_from_slice(&mac);
    id
}

pub fn setup_iface(ifindex: u32) -> Result<(), Box<dyn Error>> {
    let sock = NlSocket::connect(NlFamily::Route, None, Groups::empty())?;
    let ifmsg = IfinfomsgBuilder::default()
        .ifi_family(RtAddrFamily::Unspecified)
        .ifi_type(Arphrd::Ether)
        .ifi_index(ifindex as i32)
        .ifi_change(Iff::UP)
        .ifi_flags(Iff::empty())
        .build()?;

    let nl_msghdr = NlmsghdrBuilder::default()
        .nl_flags(NlmF::REQUEST | NlmF::ACK)
        .nl_type(Rtm::Setlink)
        .nl_payload(NlPayload::Payload(ifmsg))
        .build()?;

    let mut buf = Cursor::new(Vec::<u8>::new());
    nl_msghdr.to_bytes(&mut buf)?;
    sock.send(buf.get_ref(), Msg::empty())?;
    Ok(())
}

pub fn add_addr(sock: &NlSocket, ifindex: u32, ip: Ipv4Addr) -> Result<(), Box<dyn Error>> {
    let mut rt_buf = RtBuffer::new();
    rt_buf.push(
        RtattrBuilder::default()
            .rta_type(Ifa::Local)
            .rta_payload(ip.octets().to_vec())
            .build()?,
    );
    let ifmsg = IfaddrmsgBuilder::default()
        .ifa_family(RtAddrFamily::Inet)
        .ifa_prefixlen(24)
        .ifa_scope(RtScope::Universe)
        .ifa_flags(IfaF::empty())
        .rtattrs(rt_buf)
        .ifa_index(ifindex)
        .build()?;

    let nlmsg = NlmsghdrBuilder::default()
        .nl_flags(NlmF::REQUEST | NlmF::CREATE | NlmF::ACK)
        .nl_payload(NlPayload::Payload(ifmsg))
        .nl_type(Rtm::Newaddr)
        .build()?;

    let mut buf = Cursor::new(Vec::<u8>::new());
    nlmsg.to_bytes(&mut buf)?;
    sock.send(buf.get_ref(), Msg::empty())?;
    Ok(())
}

pub fn set_default_route(
    sock: &NlSocket,
    ifindex: u32,
    gateway: Ipv4Addr,
) -> Result<(), Box<dyn Error>> {
    let mut rtbuf = RtBuffer::new();
    rtbuf.push(
        RtattrBuilder::default()
            .rta_type(Rta::Oif)
            .rta_payload(ifindex.to_ne_bytes().to_vec())
            .build()?,
    );
    rtbuf.push(
        RtattrBuilder::default()
            .rta_type(Rta::Gateway)
            .rta_payload(gateway.octets().to_vec())
            .build()?,
    );

    let rtmsg = RtmsgBuilder::default()
        .rtm_family(RtAddrFamily::Inet)
        .rtm_table(RtTable::Main)
        .rtm_protocol(Rtprot::Boot)
        .rtm_scope(RtScope::Universe)
        .rtattrs(rtbuf)
        .rtm_type(Rtn::Unicast)
        .rtm_dst_len(0)
        .rtm_src_len(0)
        .rtm_tos(0)
        .build()?;
    let nlmsg = NlmsghdrBuilder::default()
        .nl_type(Rtm::Newroute)
        .nl_flags(NlmF::REQUEST | NlmF::CREATE | NlmF::ACK)
        .nl_payload(NlPayload::Payload(rtmsg))
        .build()?;

    let mut buf = Cursor::new(Vec::<u8>::new());
    nlmsg.to_bytes(&mut buf)?;
    sock.send(buf.get_ref(), Msg::empty())?;
    Ok(())
}

pub fn get_iface_mac(ifname: &str) -> Result<[u8; 6], Box<dyn Error>> {
    let path = format!("/sys/class/net/{}/address", ifname);
    let mut mac = String::new();
    File::open(path)?.read_to_string(&mut mac)?;
    Ok(mac_to_bytes(&mac))
}

pub fn set_iface_up(socket: &NlSocket, ifindex: i32) -> Result<(), Box<dyn Error>> {
    let ifinfo = IfinfomsgBuilder::default()
        .ifi_family(RtAddrFamily::Unspecified)
        .ifi_index(ifindex)
        .ifi_change(Iff::UP | Iff::RUNNING)
        .ifi_flags(Iff::UP | Iff::RUNNING)
        .ifi_type(Arphrd::Ether)
        .build()?;

    let nlmsg = NlmsghdrBuilder::default()
        .nl_type(Rtm::Setlink)
        .nl_flags(NlmF::REQUEST | NlmF::ACK)
        .nl_payload(NlPayload::Payload(ifinfo))
        .build()?;

    let mut buf = Cursor::new(vec![]);
    nlmsg.to_bytes(&mut buf)?;
    socket.send(buf.get_ref(), Msg::empty())?;
    Ok(())
}

pub fn return_on_disconnect(ifindex: i32) -> Result<(), Box<dyn Error>> {
    let socket = NlSocket::connect(
        NlFamily::Route,
        None,
        Groups::new_groups(&[RTMGRP_LINK as u32]),
    )?;

    loop {
        let mut buf = [0u8; 2048];
        let (size, _) = socket.recv(&mut buf, Msg::empty())?;
        let mut res_buf = Cursor::new(&buf[..size]);
        let res: Nlmsghdr<u16, Ifinfomsg> = Nlmsghdr::from_bytes(&mut res_buf)?;

        if let NlPayload::Err(e) = res.nl_payload() {
            return Err(format!("Kernel Error: {}", e).into());
        }
        if res.nl_type() == &u16::from(Rtm::Newlink)
            && let NlPayload::Payload(payload) = res.nl_payload()
            && payload.ifi_index() == &ifindex
        {
            let flags = payload.ifi_flags();
            let is_up = flags.contains(Iff::UP);
            let is_running = flags.contains(Iff::RUNNING);
            if !is_up || !is_running {
                println!("Interface [{}] is not up or running", ifindex);
                return Ok(());
            }
        }
    }
}

pub fn remove_lease_and_gateway_ip(
    ifindex: u32,
    ip_addr: Ipv4Addr,
    gateway_ip: Ipv4Addr,
    prefix_len: u8,
) -> Result<(), Box<dyn Error>> {
    let socket = NlSocket::connect(NlFamily::Route, None, Groups::empty())?;

    let ip_bytes = ip_addr.octets();
    let rtattr = RtattrBuilder::default()
        .rta_type(Ifa::Local)
        .rta_payload(ip_bytes.as_slice())
        .build()?;

    let mut rtbuf: RtBuffer<Ifa, Buffer> = RtBuffer::new();
    rtbuf.push(rtattr);

    // Building the interface structure
    let ifaddrmsg = IfaddrmsgBuilder::default()
        .ifa_family(RtAddrFamily::Inet) // IP V4
        .ifa_prefixlen(prefix_len)
        .ifa_flags(IfaF::empty())
        .ifa_scope(RtScope::Universe)
        .ifa_index(ifindex)
        .rtattrs(rtbuf)
        .build()?;

    // Building message
    let nlmsg = NlmsghdrBuilder::default()
        .nl_type(Rtm::Deladdr)
        .nl_flags(NlmF::REQUEST | NlmF::ACK)
        .nl_payload(NlPayload::Payload(ifaddrmsg.clone()))
        .build()?;

    let mut cmd_buf = Cursor::new(Vec::new());
    nlmsg.to_bytes(&mut cmd_buf)?;
    socket.send(cmd_buf.get_ref(), Msg::empty())?;
    println!("Removing...");

    let wait_for_ack = |socket: &NlSocket| -> Result<(), Box<dyn Error>> {
        loop {
            let mut res_buf = [0u8; 4096];
            let (size, _) = socket.recv(&mut res_buf, Msg::empty())?;
            let mut slice = Cursor::new(Vec::from(&res_buf[..size]));

            let msg: Nlmsghdr<u16, Nlmsgerr<u16>> = Nlmsghdr::from_bytes(&mut slice)?;
            if let NlPayload::Payload(err) = msg.nl_payload() {
                if *err.error() == 0 {
                    return Ok(());
                } else {
                    println!(
                        "Successfully removed IP {} for Interface {}",
                        ip_addr, ifindex
                    );
                    return Err(io::Error::from_raw_os_error(-err.error()).into());
                }
            }

            if *msg.nl_type() == libc::NLMSG_DONE as u16 {
                return Ok(());
            }
        }
    };

    wait_for_ack(&socket)?;
    cmd_buf.get_mut().clear();
    cmd_buf.set_position(0);

    // REMOVING GATEWAY IP
    let gw_bytes = gateway_ip.octets();
    let mut rtattrs = RtBuffer::new();
    rtattrs.push(
        RtattrBuilder::default()
            .rta_type(Rta::Gateway)
            .rta_payload(gw_bytes.as_slice())
            .build()?,
    );
    let rtmsg = RtmsgBuilder::default()
        .rtm_family(RtAddrFamily::Inet)
        .rtm_dst_len(0)
        .rtm_src_len(0)
        .rtm_tos(0)
        .rtm_table(RtTable::Unspec)
        .rtm_protocol(Rtprot::Unspec)
        .rtm_scope(RtScope::Universe)
        .rtm_type(Rtn::Unicast)
        .rtm_flags(RtmF::empty())
        .rtattrs(rtattrs)
        .build()?;

    let nlmsg = NlmsghdrBuilder::default()
        .nl_type(Rtm::Delroute)
        .nl_flags(NlmF::REQUEST | NlmF::ACK)
        .nl_payload(NlPayload::Payload(rtmsg))
        .build()?;

    nlmsg.to_bytes(&mut cmd_buf)?;
    socket.send(cmd_buf.get_ref(), Msg::empty())?;
    wait_for_ack(&socket)?;

    Ok(())
}

// This is for managing the lease connection in a separate thread
// i.e: rebinding leaase
pub fn manage_lease_thread(iface: &Interface) -> Result<(), Box<dyn Error>> {
    let iface = iface.clone();
    tokio::spawn(async move {
        let mut unicast_renewed = false;
        let mut broadcast_renewed = false;
        let ifname = iface.ifname.as_ref().unwrap().to_string();
        loop {
            match DhcpStorage::read_file() {
                Ok(files) => {
                    if files.is_empty() {
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        continue;
                    };
                    if let Some(content) = files.iter().find(|f| f.ifname == ifname) {
                        // actual absolute time the DhcpFile was initiated at
                        let time_init = content.time_initiated;

                        // duration of the lease lifetime
                        let ls_dur = content.lease_duration as i64;
                        println!("new dhcp connection");
                        loop {
                            manage_lease(
                                &iface,
                                time_init,
                                ls_dur,
                                &mut unicast_renewed,
                                &mut broadcast_renewed,
                            );
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    } else {
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
                Err(e) => {
                    println!("Error in reading dhcp files: {}", e);
                }
            }
        }
    });

    println!("Setup DHCP Lease Management Thread");
    Ok(())
}

fn manage_lease(
    iface: &Interface,
    time_init: i64,
    ls_dur: i64,
    uni_ren: &mut bool,
    brd_ren: &mut bool,
) {
    let now = Utc::now();
    let t1 = ls_dur / 2;
    let t2 = ls_dur as f64 * 0.875;
    let time_left = now.timestamp() - time_init;

    let ifname = iface.ifname.as_ref().expect("No Ifname found.").to_string();

    let data = {
        if time_left > t1 && time_left < t2 as i64 && !*uni_ren {
            *uni_ren = true;
            *brd_ren = false;
            renew_connection(iface, false)
        } else if time_left > t2 as i64 && !*brd_ren {
            *uni_ren = false;
            *brd_ren = true;
            renew_connection(iface, true)
        } else {
            Err("Nothing happened.".into())
        }
    };

    if let Ok(Some(data)) = data {
        let _ = DhcpStorage::write_from_dhcplease(&data, ifname);
    };
}

pub fn autoconnect(
    hosts: &[Host],
    iface: &Interface,
    reject_list: &[String],
    connected: &mut bool,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Seeing if any host is already connected
    if hosts.iter().find(|h| h.is_connected).is_some() {
        *connected = true;
        return Ok(());
    }
    if *connected {
        return Ok(());
    }

    // Checking if host is already in reject list
    let conpas = {
        let saved_connections = list_saved_networks().unwrap_or_default();
        let mut connection: Option<Host> = None;
        let mut pass: Option<String> = None;

        for ahost in hosts.iter() {
            let Some(ref ssid) = ahost.ssid else { continue };
            if reject_list.iter().any(|f| f == ssid) {
                continue;
            }
            let Some(ref bssid) = ahost.bssid else { continue };
            for shost in saved_connections.iter() {
                if &shost.bssid == bssid {
                    connection = Some(ahost.clone());
                    pass = Some(shost.password.clone());
                    break;
                } else {
                    *connected = false;
                }
            }
        }
        (connection, pass)
    };
    if let (Some(host), Some(password)) = conpas {
        let ssid_display = host.ssid.as_deref().unwrap_or("?");
        println!("Found Saved Network.\n Connecting to {}", ssid_display);
        let iface = iface.clone();
        if let Err(e) = connect_to(&iface, host, &Some(password), None) {
            println!("Connection Error: {}", e);
        };
        Ok(())
    } else {
        println!("No Saved Connection found.");
        Err("No Saved Network found.".into())
    }
}
