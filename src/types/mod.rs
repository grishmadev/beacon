#[derive(Debug, Default, Clone)]
pub struct Host {
    pub bssid: Option<String>,
    pub ssid: Option<String>,
    pub frequency: Option<u32>,
    pub signal: Option<u32>,
}

impl Host {
    pub fn new() -> Self {
        Self {
            bssid: None,
            ssid: None,
            frequency: None,
            signal: None,
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
