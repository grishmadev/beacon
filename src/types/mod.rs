#[derive(Debug, Default, Clone)]
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

pub struct FamilyInfo {
    pub name: String,
    pub id: u16,
}

#[derive(Debug, Clone, Default)]
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
