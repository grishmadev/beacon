use std::error::Error;

use beacon::{
    mac_to_bytes,
    wifi::{self, display_hosts, helper::get_interface},
};
fn main() -> Result<(), Box<dyn Error>> {
    println!("Hello Rust!");
    let current_connection = wifi::get_current_connection()?;
    println!("current_connection: {:#?}", current_connection);
    let hosts = wifi::get_active_networks()?;
    display_hosts(hosts.clone());
    let host = hosts[0].clone();
    let ssid = host.ssid.unwrap();
    println!("ssid: {:02X?}", ssid.as_bytes());
    let family_info = wifi::helper::get_family_info()?;
    let iface = get_interface(family_info.id)?[0].clone();
    let ifname = iface.ifname.unwrap();
    let mac = mac_to_bytes(&iface.mac.unwrap());
    wifi::wpa_supplicant::connect(mac, &ifname, &ssid, Some("kakakakaka"))?;
    Ok(())
}
