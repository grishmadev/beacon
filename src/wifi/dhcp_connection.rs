use std::{
    error::Error,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    net::Ipv4Addr,
    path::Path,
};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{DHCPINFO_PATH, types::DhcpLease};

#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Clone)]
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
    pub fn read_file() -> Result<Vec<DhcpFile>, Box<dyn Error>> {
        let path = Path::new(DHCPINFO_PATH);
        if !path.exists() || fs::metadata(path)?.len() == 0 {
            return Ok(Vec::new());
        }
        let content = fs::read(path)?;
        let dhcp_lease: Vec<DhcpFile> = bincode::deserialize(&content)?;

        Ok(dhcp_lease)
    }

    pub fn write_file(content: &mut DhcpFile) -> Result<(), Box<dyn Error>> {
        let path = Path::new(DHCPINFO_PATH);
        let _ = DhcpStorage::empty_out();
        content.time_initiated = Utc::now().timestamp();
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)?;
        let serialized = bincode::serialize(&content)?;
        file.write_all(&serialized)?;
        file.sync_all()?;
        Ok(())
    }
    pub fn write_from_dhcplease(data: &DhcpLease) -> Result<(), Box<dyn Error>> {
        println!("dhcp info: {:#?}", data);
        let mut content = DhcpFile {
            ip_addr: data.ip_addr,
            subnet_mask: data.subnet_mask,
            gateway: data.gateway,
            dns_servers: data.dns_servers.to_owned(),
            lease_duration: data.lease_duration,
            server_id: data.server_id,
            time_initiated: Utc::now().timestamp(),
        };
        DhcpStorage::write_file(&mut content)?;
        Ok(())
    }

    pub fn empty_out() -> Result<(), Box<dyn Error>> {
        let path = Path::new(DHCPINFO_PATH);
        let _ = fs::remove_file(path);
        Ok(())
    }
}
