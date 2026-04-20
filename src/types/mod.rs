use std::net::Ipv4Addr;

use dhcp4r::packet::Packet;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Host {
    pub bssid: Option<String>,
    pub ssid: Option<String>,
    pub frequency: Option<u32>,
    pub signal: Option<u32>,
    pub is_connected: bool,
}

impl Host {
    pub fn new() -> Self {
        Self {
            bssid: None,
            ssid: None,
            frequency: None,
            signal: None,
            is_connected: false,
        }
    }

    pub fn set_bssid(&mut self, bssid: String) {
        self.bssid = Some(bssid);
    }

    pub fn set_ssid(&mut self, ssid: String) {
        self.ssid = Some(ssid);
    }

    pub fn set_frequency(&mut self, frequency: u32) {
        self.frequency = Some(frequency);
    }

    pub fn set_signal(&mut self, signal: u32) {
        self.signal = Some(signal);
    }
}

#[derive(Debug, Default, PartialEq, Clone, Deserialize, Serialize)]
pub struct Connection {
    pub ssid: String,
    pub bssid: String,
    pub password: String,
}

#[derive(Debug, Clone, Default)]
pub struct FamilyInfo {
    pub name: String,
    pub id: u16,
    pub scan_group_id: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Interface {
    pub ifindex: Option<u32>,
    pub ifname: Option<String>,
    pub mac: Option<String>,
}

impl Interface {
    pub fn new() -> Self {
        Interface::default()
    }

    pub fn set_ifindex(&mut self, ifindex: u32) {
        self.ifindex = Some(ifindex);
    }

    pub fn set_ifname(&mut self, ifname: String) {
        self.ifname = Some(ifname);
    }

    pub fn set_mac(&mut self, mac: String) {
        self.mac = Some(mac);
    }
}

#[derive(Debug, Default, Clone)]
pub struct CurrentConnection {
    pub ifname: Option<String>,
    pub ssid: Option<String>,
    pub mac: Option<String>,
    pub bssid: Option<String>,
    pub frequency: Option<u32>,
}

impl CurrentConnection {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug)]
pub struct DhcpLease {
    pub ip_addr: Option<Ipv4Addr>,
    pub subnet_mask: Option<Ipv4Addr>,
    pub gateway: Option<Ipv4Addr>,
    pub dns_servers: Vec<Ipv4Addr>,
    pub server_id: Option<Ipv4Addr>,
    pub lease_duration: u32,
    pub renewal_time: u32,
    pub rebinding_time: u32,
    pub offer: Packet,
}
