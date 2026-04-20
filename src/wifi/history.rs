use std::{error::Error, fs, net::Ipv4Addr};

use crate::{HISTORY_PATH, types::Connection};

pub fn list_saved_networks() -> Result<Vec<Connection>, Box<dyn Error>> {
    // Check if exists; if not, return empty Vec instead of writing immediately
    if !std::path::Path::new(HISTORY_PATH).exists() {
        return Ok(Vec::new());
    }

    let file = fs::read(HISTORY_PATH)?;
    // Handle empty files gracefully
    if file.is_empty() {
        return Ok(Vec::new());
    }

    let serialized: Vec<Connection> = serde_json::from_slice(&file)?;
    Ok(serialized)
}

pub fn add_connection_to_history(connection: Connection) -> Result<(), Box<dyn Error>> {
    let mut connections = list_saved_networks()?;

    // Check for duplicates before pushing so your list doesn't grow infinitely
    if !connections.iter().any(|c| c.bssid == connection.bssid) {
        connections.push(connection);
        save_to_disk(&connections)?;
    }

    Ok(())
}

pub fn delete_connection_from_history(bssid: Ipv4Addr) -> Result<(), Box<dyn Error>> {
    let mut connections = list_saved_networks()?;

    let original_len = connections.len();
    connections.retain(|c| c.bssid != bssid.to_string());

    // Only write if something actually changed
    if connections.len() != original_len {
        save_to_disk(&connections)?;
    }

    Ok(())
}

fn save_to_disk(connections: &Vec<Connection>) -> Result<(), Box<dyn Error>> {
    let content = serde_json::to_string_pretty(connections)?;
    fs::write(HISTORY_PATH, content)?;
    Ok(())
}
