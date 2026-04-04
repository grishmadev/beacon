mod helper;
use std::error::Error;

use crate::wifi::helper::{get_family_info, get_scan};

pub fn scan_wifi_networks() -> Result<(), Box<dyn Error>> {
    let family_info = get_family_info()?;
    get_scan(family_info.id)?;
    Ok(())
}
