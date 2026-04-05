mod helper;
use std::error::Error;

use crate::{
    types::Host,
    wifi::helper::{get_family_info, get_scan},
};

pub fn scan_wifi_networks() -> Result<(), Box<dyn Error>> {
    let family_info = get_family_info()?;
    let hosts = get_scan(family_info.id)?;
    display_hosts(hosts);
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
