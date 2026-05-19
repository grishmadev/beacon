use serde::{Deserialize, Serialize};

use crate::types::{Connection, CurrentConnection, Host, Interface};
pub mod backend;
pub mod debug;
pub mod executer;
pub mod frontend;
pub mod types;
pub mod wifi;

pub fn mac_to_bytes(mac: &str) -> [u8; 6] {
    // mac in format 12:34:56:78:90:12
    let mac_arr = mac.split(":");
    let mut res = [0u8; 6];
    println!("mac: {}", mac);
    for (i, m) in mac_arr.enumerate() {
        res[i] = u8::from_str_radix(m, 16)
            .map_err(|e| format!("Invalid hex at index {}: {}", i * 2, e))
            .unwrap();
    }
    res
}
/*
********************************
*           GLOBAL TYPES        *
*********************************
*/

pub const SOCKET_PATH: &str = "/run/beacon.sock";
pub const HISTORY_PATH: &str = "/var/beacon_history.json";
pub const DHCPINFO_PATH: &str = "/var/beacon_dhcp_info";

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub enum Command {
    Ping,
    Tick,
    ListConnections,
    ListActiveConnections(Interface),
    CurrentConnection,
    ListInterfaces,
    Connect {
        bssid: String,
        password: Option<String>,
        iface: Interface,
    },
    Notification(String),
    ClearNotification,
    Disconnect,
    Info(String), // bssid,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum Response {
    Ok,
    Pong,
    Tick,
    CurrentConnection(Option<CurrentConnection>),
    ActiveHosts(String, Vec<Host>),
    SavedHosts(Vec<Connection>),
    AllInterfaces(Vec<Interface>),
    Notification(String),
    Connected,
    Disconnected,
    ClearNotification,
    Error(String),
}
