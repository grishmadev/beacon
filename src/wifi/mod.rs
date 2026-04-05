pub mod helper;
use std::error::Error;

use crate::{
    types::{Host, Interface},
    wifi::helper::{get_family_info, get_interface, get_scan},
};

pub fn scan_wifi_networks() -> Result<(), Box<dyn Error>> {
    let family_info = get_family_info()?;
    let interfaces = get_interface(family_info.id)?;
    display_interfaces(&interfaces);
    for iface in interfaces {
        let hosts = get_scan(family_info.id, iface.ifindex.unwrap())?;
        display_hosts(hosts);
    }
    Ok(())
}

fn display_hosts(hosts: Vec<Host>) {
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
