use std::{error::Error, net::Ipv4Addr};

use crate::{
    mac_to_bytes,
    types::{Connection, FamilyInfo, Host, Interface, InterfaceType},
    wifi::{
        helper::{get_interfaces, get_scan, trigger_scan},
        history::{add_connection_to_history, list_saved_networks},
        wpa_supplicant::{connect, disconnect},
    },
};

pub fn list_active_signals(
    family_info: &FamilyInfo,
    interfaces: &Vec<Interface>,
) -> Result<Vec<Host>, Box<dyn Error>> {
    let mut result = vec![];
    let family_id = family_info.id;
    for iface in interfaces {
        if iface.iftype != InterfaceType::Wireless {
            continue;
        }
        let ifindex = iface.ifindex.unwrap();
        trigger_scan(family_info, ifindex)?;
        let hosts = get_scan(family_id, ifindex)?;
        result.extend(hosts);
    }
    Ok(result)
}

pub fn list_all_signals() -> Result<Vec<Connection>, Box<dyn Error>> {
    let networks = list_saved_networks()?;
    Ok(networks)
}

pub async fn connect_to(
    family_info: &FamilyInfo,
    interfaces: &Vec<Interface>,
    iface: &Interface,
    bssid: &str,
    password: &Option<String>,
) -> Result<(), Box<dyn Error>> {
    let hosts = list_active_signals(family_info, interfaces)?;
    let target = hosts
        .iter()
        .find(|e| e.bssid.as_deref() == Some(bssid))
        .ok_or("No such Connection Found.")?;
    let mac_address = iface.mac.as_ref().ok_or("MAC Address not found.")?;
    let mac_bytes = mac_to_bytes(mac_address);
    let ifname = iface.ifname.as_ref().ok_or("Interface Name not found.")?;
    let ifindex = iface.ifindex.as_ref().ok_or("Interface Index not found.")?;
    let saved_networks = list_all_signals()?;
    let ssid = target.ssid.as_ref().ok_or("Target SSID missing.")?;
    let found_password_option = saved_networks.iter().find(|e| &e.ssid == ssid);
    let final_password: String;
    match password {
        Some(val) => {
            final_password = val.to_string();
        }
        None => match found_password_option {
            Some(conn) => {
                final_password = conn.password.to_string();
            }
            None => {
                return Err("Password not provided. Please provide a password.".into());
            }
        },
    }
    let bssid = target.bssid.as_ref().ok_or("Target BSSID missing.")?;
    match connect(mac_bytes, ifname, ifindex, ssid, &final_password).await {
        Ok(_) => {
            // saving connection
            let connection = Connection {
                ssid: ssid.to_string(),
                bssid: bssid.to_string(),
                password: final_password,
            };
            add_connection_to_history(connection)?;
        }
        Err(e) => {
            return Err(e);
        }
    };
    Ok(())
}

pub fn disconnect_connection(ifname: &str) -> Result<(), Box<dyn Error>> {
    disconnect(ifname)?;
    Ok(())
}

pub fn list_interfaces() -> Result<Vec<Interface>, Box<dyn Error>> {
    let interfaces = get_interfaces()?;
    Ok(interfaces)
}
