use crate::types::{CurrentConnection, FamilyInfo, Host, Interface, InterfaceType};
use neli::{
    FromBytes, ToBytes,
    attr::Attribute,
    consts::{
        genl::{CtrlAttr, CtrlAttrMcastGrp, CtrlCmd},
        nl::{GenlId, NlmF, Nlmsg},
        rtnl::{Arphrd, Iff, Ifla, RtAddrFamily, RtScope, RtTable, Rta, Rtm, Rtn, Rtprot},
        socket::{Msg, NlFamily},
    },
    genl::{AttrType, AttrTypeBuilder, Genlmsghdr, GenlmsghdrBuilder, Nlattr, NlattrBuilder},
    nl::{NlPayload, Nlmsghdr, NlmsghdrBuilder},
    rtnl::{Ifinfomsg, IfinfomsgBuilder, RtmsgBuilder},
    socket::NlSocket,
    types::{Buffer, GenlBuffer, RtBuffer},
    utils::Groups,
};
use std::{
    error::Error,
    io::Cursor,
    path::{self, Path},
};

use nl80211::{Nl80211Attr, Nl80211Bss, Nl80211Cmd};

pub fn get_family_info() -> Result<FamilyInfo, Box<dyn Error>> {
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
                let name_arr: [u8; 8] = attr.get_payload_as()?;
                let name = name_arr
                    .iter()
                    .map(|x| *x as char)
                    .collect::<String>()
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

pub fn get_scan(family_id: u16, ifindex: u32) -> Result<Vec<Host>, Box<dyn Error>> {
    let mut result = Vec::<Host>::new();
    let sock = NlSocket::new(NlFamily::Generic)?;
    // Read the interface card

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
        let res: Nlmsghdr<GenlId, Genlmsghdr<CtrlCmd, u16>> = Nlmsghdr::from_bytes(&mut cursor)?;

        if let NlPayload::Err(e) = res.nl_payload() {
            return Err(format!("Kernel Error: {}", e).into());
        }

        if u16::from(*res.nl_type()) == libc::NLMSG_DONE as u16 {
            break;
        }

        if let NlPayload::Payload(genl) = res.nl_payload() {
            let attrs = genl.attrs();

            for attr in attrs.iter() {
                if Nl80211Attr::from(*attr.nla_type().nla_type()) == Nl80211Attr::AttrBss {
                    let bss_bytes = attr.nla_payload().as_ref();

                    let mut cursor = Cursor::new(bss_bytes);
                    // parsing the nested byte as a flatlsit
                    // initialize Host
                    let mut target = Host::new();

                    while cursor.position() < bss_bytes.len() as u64 {
                        if let Ok(nested) = Nlattr::<u16, Buffer>::from_bytes(&mut cursor) {
                            match Nl80211Bss::from(*nested.nla_type().nla_type()) {
                                Nl80211Bss::BssBssid => {
                                    let bytes = nested.nla_payload().as_ref();
                                    // Mac Address
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
                                        // kernel returns milli-dBm
                                        let signal =
                                            u32::from_le_bytes(bytes[..4].try_into()?) / 100;
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
                                            break;
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
                        } else {
                            break;
                        }
                        // add target to result
                    }
                    println!("host: {:#?}", target);
                    result.push(target);
                }
            }
        }
    }
    Ok(result)
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

            if u16::from(*res.nl_type()).to_string() == libc::NLMSG_DONE.to_string() {
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
                            let payload = attr.rta_payload().as_ref();
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
        let res: Nlmsghdr<u16, Genlmsghdr<u8, u16>> = Nlmsghdr::from_bytes(&mut cursor)?;

        if let NlPayload::Err(e) = res.nl_payload() {
            return Err(format!("Kernel Error: {}", e).into());
        }

        if *res.nl_type().to_string() == libc::NLMSG_DONE.to_string() {
            break;
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

                    Nl80211Attr::AttrMac => {
                        if payload.len() >= 6 {
                            let mac = payload[..6]
                                .iter()
                                .map(|b| format!("{b:02X}"))
                                .collect::<Vec<_>>()
                                .join(":");
                            connection.mac = Some(mac);
                        }
                    }

                    Nl80211Attr::AttrIfindex => {
                        if payload.len() >= 4 {
                            let ifindex = u32::from_le_bytes(payload[..4].try_into()?);
                            let hosts = get_scan(family_id, ifindex)?;
                            match hosts.into_iter().find(|h| h.is_connected) {
                                Some(host) => {
                                    connection.ssid = host.ssid;
                                    connection.bssid = host.bssid;
                                    connection.frequency = host.frequency;
                                }
                                None => return Ok(None),
                            }
                        }
                    }

                    _ => {
                        println!("{:#?}", String::from_utf8_lossy(payload))
                    }
                }
            }
            return Ok(Some(connection));
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
        .nla_payload(ifindex)
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
            return Err(format!("Kernel Error: {}", e).into());
        }

        if let NlPayload::Payload(genl) = res.nl_payload() {
            match Nl80211Cmd::from(*genl.cmd()) {
                Nl80211Cmd::CmdNewScanResults => {
                    // scanning finished, new results in cache
                    println!("Scan Complete...");
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

// pub fn save_connection() -> Resul
