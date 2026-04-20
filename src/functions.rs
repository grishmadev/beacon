use std::{error::Error, net::Ipv4Addr};

use crate::{
    types::{Connection, Host},
    wifi::{
        helper::{get_family_info, get_interface, get_scan, trigger_scan},
        history::list_saved_networks,
    },
};

pub fn list_active_signals() -> Result<Vec<Host>, Box<dyn Error>> {
    let family_info = get_family_info()?;
    let mut result = vec![];
    let family_id = family_info.id;
    let ifaces = get_interface(family_id)?;
    for iface in ifaces {
        let ifindex = iface.ifindex.unwrap();
        trigger_scan(&family_info, ifindex)?;
        let hosts = get_scan(family_id, ifindex)?;
        result.extend(hosts);
    }
    Ok(result)
}

pub fn list_signals() -> Result<Vec<Connection>, Box<dyn Error>> {
    let networks = list_saved_networks()?;
    Ok(networks)
}

pub fn connect_to(bsid: Ipv4Addr) -> Result<(), Box<dyn Error>> {
    Ok(())
}
