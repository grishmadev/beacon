use std::{
    error::Error,
    fs::{File, OpenOptions},
    io::{Read, Write},
    net::Ipv4Addr,
    path::Path,
};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::DHCPINFO_PATH;

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct DhcpFile {
    pub ip_addr: Option<Ipv4Addr>,
    pub subnet_mask: Option<Ipv4Addr>,
    pub gateway: Option<Ipv4Addr>,
    pub dns_servers: Vec<Ipv4Addr>,
    pub server_id: Option<Ipv4Addr>,
    pub lease_duration: u32,
    pub time_initiated: i64,
}

pub struct DhcpStorage;
impl DhcpStorage {
    pub fn new() -> Result<(), Box<dyn Error>> {
        OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(DHCPINFO_PATH)?;
        Ok(())
    }
    pub fn read_file() -> Result<Option<DhcpFile>, Box<dyn Error>> {
        let mut res_buf = vec![];
        let mut file = OpenOptions::new().read(true).open(DHCPINFO_PATH)?;
        let size = File::read(&mut file, &mut res_buf)?;
        let content = &res_buf[..size];
        if res_buf.is_empty() {
            return Ok(None);
        };
        let dhcp_lease: DhcpFile = bincode::deserialize(content)?;

        Ok(Some(dhcp_lease))
    }

    pub fn write_file(content: &mut DhcpFile) -> Result<(), Box<dyn Error>> {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(DHCPINFO_PATH)?;
        if !Path::new(DHCPINFO_PATH).exists() {
            content.time_initiated = Utc::now().timestamp();
            let serialized = bincode::serialize(&content)?;
            file.write_all(&serialized)?;
        }
        Ok(())
    }

    pub fn empty_out(&mut self) -> Result<(), Box<dyn Error>> {
        File::create(Path::new(DHCPINFO_PATH))?;
        Ok(())
    }
}
