use beacon::{
    Command, Response,
    backend::functions::list_interfaces,
    executer::execute,
    mac_to_bytes,
    types::InterfaceType,
    wifi::{
        dhcp_connection::DhcpStorage, helper::manage_lease_thread,
        wpa_supplicant::connect_via_ethernet,
    },
};
use chrono::Utc;
use std::{
    error::Error,
    fs,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixListener,
};

use beacon::SOCKET_PATH;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Clean up old socket file if it exists
    let _ = fs::remove_file(SOCKET_PATH);

    println!("Server Started. :D\nDaemon listening on {}", SOCKET_PATH);

    let connected_via_ether = Arc::new(AtomicBool::new(false));
    let status_clone = Arc::clone(&connected_via_ether);
    tokio::spawn(async move {
        loop {
            let is_connected = status_clone.load(Ordering::SeqCst);
            let ifaces = list_interfaces().unwrap_or_default();

            let eth = ifaces
                .iter()
                .find(|iface| iface.iftype == InterfaceType::Wired);

            match (is_connected, eth) {
                (false, Some(f)) if connect_via_ethernet(f).is_ok() => {
                    println!("Connected via Ethernet");
                    status_clone.store(true, Ordering::SeqCst);
                }
                (true, None) => {
                    println!("Disconnected Ethernet");
                    status_clone.store(false, Ordering::SeqCst);
                }
                _ => {}
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });

    // Spawning thread to check for residue leases and manage them
    tokio::spawn(async move {
        println!("Checking for previous lease");
        let files = DhcpStorage::read_file().unwrap();
        let interfaces = list_interfaces().unwrap();
        if files.is_empty() {
            println!("No DhcpLease found.");
            return;
        };
        println!("Found Residue Dhcp");
        let file = files.first().unwrap();
        if file.time_initiated + file.lease_duration as i64 <= Utc::now().timestamp() {
            println!("Found Invalid Dhcp");
            let _ = DhcpStorage::empty_out();
            return;
        };
        let iface = interfaces
            .iter()
            .find(|f| f.ifname.as_ref().unwrap() == &file.ifname)
            .unwrap();
        println!("Spawned Thread management for current DhcpLease");
        let _ = manage_lease_thread(iface);
    });

    let listener = UnixListener::bind(SOCKET_PATH).unwrap();
    loop {
        let (mut socket, _) = match listener.accept().await {
            Ok(s) => s,
            Err(_) => {
                continue;
            }
        };
        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            loop {
                match socket.read(&mut buf).await {
                    Ok(0) => {
                        break;
                    }
                    Ok(n) => {
                        let cmd: Command = bincode::deserialize(&buf[..n]).unwrap();
                        let response = match execute(&cmd).await {
                            Ok(s) => s,
                            Err(e) => Response::Error(e.to_string()),
                        };
                        let serialized = bincode::serialize(&response).unwrap();
                        socket
                            .write_all(&serialized)
                            .await
                            .expect("Couldn't Write to File");
                    }
                    Err(e) => {
                        eprint!("Socket Error: {}", e);
                        break;
                    }
                };
            }
        });
    }
}
