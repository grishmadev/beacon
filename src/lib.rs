use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs::File;

use crate::types::{Connection, Host, Interface};
pub mod backend;
pub mod frontend;
pub mod types;
pub mod wifi;

pub fn mac_to_bytes(mac: &str) -> [u8; 6] {
    // mac in format 12:34:56:78:90:12
    let mac_arr = mac.split(":");
    let mut res = [0u8; 6];
    for (i, m) in mac_arr.enumerate() {
        res[i] = u8::from_str_radix(m, 16)
            .map_err(|e| format!("Invalid hex at index {}: {}", i * 2, e))
            .unwrap();
    }
    res
}
/*
*********************************
*           GLOBAL TYPES        *
*********************************
*/

pub const SOCKET_PATH: &str = "/run/beacon.sock";
pub const HISTORY_PATH: &str = "/var/beacon_history.json";

#[derive(Deserialize, Serialize, Debug)]
pub enum Command {
    Ping,
    ListConnections,
    ListActiveConnections,
    ListInterfaces,
    Connect {
        bssid: String,
        password: Option<String>,
        iface: Interface,
    },
    Disconnect,
    Info(String), // bssid,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum Response {
    Ok,
    Pong,
    ActiveHosts(Vec<Host>),
    SavedHosts(Vec<Connection>),
    AllInterfaces(Vec<Interface>),
    Error(String),
}

// pub mod debug {
//     use std::error::Error;
//     use std::io::Read;
//     use std::os::unix::fs::FileExt;
//     use std::{
//         fs::{self, File},
//         path::Path,
//     };
//
//     pub fn write(str: &str) -> Result<(), Box<dyn Error>> {
//         let path = "./debug.txt";
//         if !Path::new(path).exists() {
//             fs::File::create(path)?;
//         }
//         let mut file = File::open(path)?;
//         let mut content: String = "".to_string();
//         let endpos = content.len();
//         file.read_to_string(&mut content)?;
//         file.write_at(str.as_bytes(), endpos as u64)?;
//         Ok(())
//     }
// }
