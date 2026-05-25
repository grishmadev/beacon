use std::{
    error::Error,
    sync::{Arc, Mutex},
};

use crate::{
    types::{Connection, CurrentConnection, FamilyInfo, Host, Interface, InterfaceType},
    wifi::{
        helper::{get_current, get_family_info, get_interfaces, get_scan, trigger_scan},
        history::{add_connection_to_history, list_saved_networks},
        wpa_supplicant::{connect, disconnect},
    },
};

pub fn list_active_signals(
    family_info: &FamilyInfo,
    interface: Interface,
) -> Result<Vec<Host>, Box<dyn Error + Send + Sync>> {
    let mut result = vec![];
    let family_id = family_info.id;
    if interface.iftype != InterfaceType::Wireless {
        return Ok(Vec::new());
    }
    let ifindex = interface.ifindex.unwrap();
    trigger_scan(family_info, ifindex).unwrap();
    let hosts = get_scan(family_id, ifindex)?;
    // let logs = format!(
    //     "hosts for {:?} {:?}",
    //     interface.ifname.clone(),
    //     hosts.clone()
    // );
    // println!("{}", logs);
    result.extend(hosts);
    Ok(result)
}

// pub fn list_and_connect() -> Result<(), ()> {
//     let hosts = list_active_signals(family_info, interface)?;
//     let saved_hosts = list_saved_networks()?;
//     for host in hosts {
//         match saved_hosts
//             .iter()
//             .find(|h| h.bssid == host.bssid.as_ref().unwrap().to_string())
//         {
//             Some(connection) => {}
//             None => {}
//         };
//     }
//     Ok(())
// }

pub fn list_all_signals() -> Result<Vec<Connection>, Box<dyn Error>> {
    let networks = list_saved_networks()?;
    Ok(networks)
}

pub fn connect_to(
    iface: &Interface,
    host: Host,
    password: &Option<String>,
    reject_list: Option<Arc<Mutex<Vec<String>>>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let saved_networks = list_all_signals().unwrap_or_default();

    let found_password_option = saved_networks
        .iter()
        .find(|e| &e.bssid == host.bssid.as_ref().unwrap());
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
    };
    let bssid = host.bssid.expect("No BSSID found.");
    let ssid = host.ssid.expect("Target SSID missing.");
    match connect(iface, &ssid, &final_password) {
        Ok(_) => {
            // saving connection
            let connection = Connection {
                ssid: ssid.to_string(),
                bssid: bssid.to_string(),
                password: final_password,
            };
            if let Some(list) = reject_list {
                let mut guard = list.lock().unwrap();
                if let Some(idx) = guard.iter().position(|f| f == &ssid) {
                    guard.remove(idx);
                }
            }
            add_connection_to_history(connection).unwrap();
        }
        Err(e) => {
            println!("Connection Error: {:#?}", e);
            return Ok(());
        }
    };
    Ok(())
}

pub fn disconnect_connection(
    ifname: &str,
    reject_list: Option<Arc<Mutex<Vec<String>>>>,
) -> Result<(), Box<dyn Error>> {
    let family_info = get_family_info().unwrap();
    if disconnect(ifname, true).is_ok() {
        if let Some(current) = get_current(family_info.id)? {
            let ssid = current.ssid.unwrap();
            if let Some(list) = reject_list {
                let mut guard = list.lock().unwrap();
                if let Some(idx) = guard.iter().position(|f| f == &ssid) {
                    guard.remove(idx);
                }
            }
        } else {
            eprintln!("No SSID Found in Saved List.");
        }
    };
    Ok(())
}

pub fn list_interfaces() -> Result<Vec<Interface>, Box<dyn Error>> {
    let interfaces = get_interfaces()?;
    Ok(interfaces)
}

pub fn current_connection() -> Result<Option<CurrentConnection>, Box<dyn Error>> {
    let family_info = get_family_info().unwrap();
    let family_id = family_info.id;
    let info = get_current(family_id)?;
    Ok(info)
}
