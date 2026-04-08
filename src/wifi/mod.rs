pub mod helper;
pub mod wpa_supplicant;
use std::error::Error;

use crate::{
    types::{CurrentConnection, Host, Interface},
    wifi::helper::{get_family_info, get_interface, get_scan, trigger_scan},
};

pub fn get_active_networks() -> Result<Vec<Host>, Box<dyn Error>> {
    let family_info = get_family_info()?;
    println!("{:#?}", family_info);
    let interfaces = get_interface(family_info.id)?;
    let mut result = Vec::<Host>::new();
    for iface in interfaces {
        let ifname = iface.ifname.unwrap_or("---".to_string());
        if let Some(ifindex) = iface.ifindex {
            println!("Scanning {}", ifname);
            trigger_scan(&family_info, ifindex)?;
            let hosts = get_scan(family_info.id, ifindex)?;
            result.extend(hosts);
        } else {
            println!("Couldn't Scan {}", ifname);
        }
    }
    Ok(result)
}

pub fn get_current_connection() -> Result<Option<CurrentConnection>, Box<dyn Error>> {
    let family_info = helper::get_family_info()?;
    let current_connection = helper::get_current(family_info.id)?;
    Ok(current_connection)
}

pub fn display_hosts(hosts: Vec<Host>) {
    println!(
        "{:<30} {:<20} {:>10} {:>12}",
        "SSID", "BSID", "Frequency(MHz)", "Signal(dBm)"
    );
    println!("{}", "=".repeat(100));

    for host in hosts {
        let ssid = host.ssid.unwrap_or("---".to_string());
        let bssid = host.bssid.unwrap_or("---".to_string());
        let frequency = match host.frequency {
            Some(s) => s.to_string(),
            None => "-".to_string(),
        };
        let signal = match host.signal {
            Some(s) => s.to_string(),
            None => "-".to_string(),
        };
        println!(
            "{:<30} {:<20} {:>10} {:>12}",
            ssid, bssid, frequency, signal
        );
        println!("{}", "-".repeat(100));
    }
}

fn display_interfaces(interfaces: &[Interface]) {
    println!("{:<20} {:<20} {:<30}", "Index", "Name", "Mac");
    println!("{}", "=".repeat(100));
    for iface in interfaces {
        let ifindex = match iface.ifindex {
            Some(s) => s.to_string(),
            None => "---".to_string(),
        };
        let ifname = iface.ifname.clone().unwrap_or("---".to_string());
        let mac = iface.mac.clone().unwrap_or("---".to_string());
        println!("{:<20} {:<20} {:<30}", ifindex, ifname, mac);
        println!("{}", "-".repeat(100));
    }
}
