use std::{
    error::Error,
    fs::{self, OpenOptions},
    io::Write,
    net::Ipv4Addr,
    path::Path,
};

use bincode::{Decode, Encode, config};
use chrono::Utc;

use crate::{DHCPINFO_PATH, types::DhcpLease};

#[derive(Debug, Default, PartialEq, Clone, Decode, Encode)]
pub struct DhcpFile {
    pub ifname: String,
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
    pub fn read_file() -> Result<Vec<DhcpFile>, Box<dyn Error + Send + Sync>> {
        let path = Path::new(DHCPINFO_PATH);
        if !path.exists() || fs::metadata(path)?.len() == 0 {
            return Ok(Vec::new());
        }
        let content = fs::read(path)?;
        let (dhcp_lease, _) = bincode::decode_from_slice(&content, config::standard())?;

        Ok(dhcp_lease)
    }

    /// Use this function when clearing out any duplicate DhcpFiles
    fn get_unique() -> Result<Vec<DhcpFile>, Box<dyn Error>> {
        let mut result: Vec<DhcpFile> = vec![];
        let files = DhcpStorage::read_file().unwrap_or_default();
        for file in files {
            if result.iter().any(|f| f.ifname == file.ifname) {
                continue;
            };
            result.push(file);
        }
        Ok(result)
    }
    pub fn read_specific(ifname: &str) -> Result<Option<DhcpFile>, Box<dyn Error>> {
        let content =
            DhcpStorage::read_file().map_err(|e| Box::<dyn Error>::from(format!("{e}")))?;
        let res = content
            .iter()
            .find(|f| f.ifname == ifname)
            .map(|f| f.to_owned());

        Ok(res)
    }

    pub fn write_file(content: &mut DhcpFile) -> Result<(), Box<dyn Error>> {
        let time = Utc::now().timestamp();
        content.time_initiated = time;
        let mut lease =
            DhcpStorage::read_file().map_err(|e| Box::<dyn Error>::from(format!("{e}")))?;
        if let Some(target_idx) = lease.iter().position(|f| f.ifname == content.ifname) {
            lease[target_idx] = content.clone();
        } else {
            lease.push(content.clone());
        };
        println!("Writing to file: {lease:#?}");
        let serialized = bincode::encode_to_vec(&lease, config::standard())?;
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(DHCPINFO_PATH)?;
        file.write_all(&serialized)?;
        file.sync_all()?;
        Ok(())
    }
    pub fn write_from_dhcplease(data: &DhcpLease, ifname: String) -> Result<(), Box<dyn Error>> {
        let mut content = DhcpFile {
            ip_addr: data.ip_addr,
            subnet_mask: data.subnet_mask,
            gateway: data.gateway,
            dns_servers: data.dns_servers.to_owned(),
            lease_duration: data.lease_duration,
            server_id: data.server_id,
            ifname,
            ..Default::default()
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
