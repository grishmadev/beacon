use crate::types::{FamilyInfo, Host};
use neli::{
    FromBytes, ToBytes,
    attr::Attribute,
    consts::{
        genl::{CtrlAttr, CtrlCmd},
        nl::{GenlId, NlmF, Nlmsg},
        socket::{Msg, NlFamily},
    },
    genl::{AttrType, AttrTypeBuilder, Genlmsghdr, GenlmsghdrBuilder, Nlattr, NlattrBuilder},
    nl::{NlPayload, Nlmsghdr, NlmsghdrBuilder},
    socket::NlSocket,
    types::{Buffer, GenlBuffer},
};
use std::{error::Error, io::Cursor};

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

    let mut family_name: Option<String> = None;
    let mut family_id: Option<u16> = None;
    if let NlPayload::Payload(genl) = res.nl_payload() {
        let attrs = genl.attrs();
        for attr in attrs.iter() {
            if *attr.nla_type().nla_type() == CtrlAttr::FamilyId {
                let id: u16 = attr.get_payload_as()?;
                family_id = Some(id);
            }
            if *attr.nla_type().nla_type() == CtrlAttr::FamilyName {
                let name_arr: [u8; 8] = attr.get_payload_as()?;
                let name = name_arr.iter().map(|x| *x as char).collect::<String>();
                family_name = Some(name);
            }
        }
    }
    if family_id.is_none() || family_name.is_none() {
        return Err("Failed to get family info".into());
    }
    Ok(FamilyInfo {
        name: family_name.unwrap(),
        id: family_id.unwrap(),
    })
}

pub fn get_scan(family_id: u16) -> Result<Vec<Host>, Box<dyn Error>> {
    let mut result = Vec::<Host>::new();
    let sock = NlSocket::new(NlFamily::Generic)?;
    // Read the interface card
    let ifindex_str = std::fs::read_to_string("/sys/class/net/wlo1/ifindex")?;
    let ifindex: u32 = ifindex_str.trim().parse()?;

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

        if u16::from(*res.nl_type()) == u16::from(Nlmsg::Done) {
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

                                _ => {}
                            }
                        } else {
                            break;
                        }
                        // add target to result
                    }
                    result.push(target);
                }
            }
        }
    }
    Ok(result)
}
