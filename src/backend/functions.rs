use std::{
    error::Error,
    sync::{Arc, Mutex},
};

use crate::{
    Log,
    debug::log_msg,
    types::{Connection, CurrentConnection, FamilyInfo, Host, Interface, InterfaceType},
    wifi::{
        helper::{get_current, get_interfaces, get_scan, trigger_scan},
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
    let ifindex = interface.ifindex.ok_or("No ifindex for interface.")?;
    trigger_scan(family_info, ifindex).map_err(|e| format!("Trigger scan failed: {}", e))?;
    let hosts = get_scan(family_id, ifindex)?;
    result.extend(hosts);
    Ok(result)
}

pub fn list_all_signals() -> Result<Vec<Connection>, Box<dyn Error>> {
    let networks = list_saved_networks()?;
    Ok(networks)
}

pub fn connect_to(
    iface: &Interface,
    host: Host,
    password: &Option<String>,
    reject_list: Option<Arc<Mutex<Vec<String>>>>,
) -> Result<(), Box<dyn Error>> {
    let saved_networks = list_all_signals().unwrap_or_default();

    let found_password_option = host
        .bssid
        .as_ref()
        .and_then(|bssid| saved_networks.iter().find(|e| &e.bssid == bssid));
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
    let bssid = host.bssid.ok_or("No BSSID found.")?;
    let ssid = host.ssid.ok_or("Target SSID missing.")?;
    match connect(iface, &ssid, &final_password) {
        Ok(_) => {
            let connection = Connection {
                ssid: ssid.to_string(),
                bssid: bssid.to_string(),
                password: final_password,
            };
            if let Some(list) = &reject_list {
                let mut guard = list.lock().map_err(|e| format!("Lock: {}", e))?;
                if let Some(idx) = guard.iter().position(|f| f == &ssid) {
                    guard.remove(idx);
                }
            }
            add_connection_to_history(connection).map_err(|e| format!("{e}"))?;
        }
        Err(e) => {
            return Err(format!("Connection Error: {:#?}", e).into());
        }
    };
    Ok(())
}

pub fn disconnect_connection(
    ifname: &str,
    reject_list: Option<Arc<Mutex<Vec<String>>>>,
) -> Result<(), Box<dyn Error>> {
    // let family_info = get_family_info().map_err(|e| format!("Failed to get family info: {}", e))?;
    if disconnect(ifname, true).is_ok() {
        if let Some(current) = get_current()? {
            let current = current
                .iter()
                .find(|f| f.ifname == Some(ifname.to_string()))
                .unwrap()
                .to_owned();
            if let Some(ssid) = current.ssid
                && let Some(list) = reject_list
            {
                let mut guard = list.lock().map_err(|e| format!("Lock: {}", e))?;
                if let Some(idx) = guard.iter().position(|f| f == &ssid) {
                    guard.remove(idx);
                }
            }
        } else {
            log_msg("No SSID Found in Saved List.", Log::Err);
        }
    };
    Ok(())
}

pub fn list_interfaces() -> Result<Vec<Interface>, Box<dyn Error>> {
    let interfaces = get_interfaces()?;
    Ok(interfaces)
}

pub fn current_connection() -> Result<Option<Vec<CurrentConnection>>, Box<dyn Error>> {
    let info = get_current()?;
    Ok(info)
}
