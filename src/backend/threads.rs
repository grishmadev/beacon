use std::{
    error::Error,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use bincode::config;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixListener,
};

use chrono::Utc;

use crate::{
    Command, Response, SOCKET_PATH,
    backend::functions::{list_active_signals, list_interfaces},
    executer::execute,
    types::InterfaceType,
    wifi::{
        dhcp_connection::DhcpStorage,
        helper::{autoconnect, get_family_info, manage_lease_thread},
        wpa_supplicant::connect_via_ethernet,
    },
};

pub fn spawn_ethernet_connection() -> Result<(), Box<dyn Error>> {
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
    Ok(())
}

// Spawning thread to check for residue leases and manage them
pub fn spawn_residue_connection() -> Result<(), Box<dyn Error>> {
    tokio::spawn(async move {
        let files = DhcpStorage::read_file().unwrap();
        let interfaces = list_interfaces().unwrap();
        if files.is_empty() {
            return;
        };
        let file = files.first().unwrap();
        if file.time_initiated + file.lease_duration as i64 <= Utc::now().timestamp() {
            println!("Found Residue Dhcp");
            let _ = DhcpStorage::empty_out();
            return;
        };
        let iface = interfaces
            .iter()
            .find(|f| f.ifname.as_ref().unwrap() == &file.ifname)
            .unwrap();
        let _ = manage_lease_thread(iface);
    });
    Ok(())
}

pub async fn spawn_autoconnection(
    reject_list: Arc<Mutex<Vec<String>>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let ifaces = list_interfaces().unwrap_or_default();
    for iface in ifaces {
        if iface.iftype != InterfaceType::Wireless {
            continue;
        }
        let reject_list = reject_list.clone();
        tokio::spawn(async move {
            let family_info = get_family_info().unwrap();
            let mut connected = false;
            loop {
                let hosts_res = list_active_signals(&family_info, iface.clone()).ok();

                if let Some(hosts) = hosts_res {
                    let list = reject_list.lock().unwrap();

                    if let Err(e) = autoconnect(&hosts, &iface, &list, &mut connected) {
                        eprintln!("Autoconnection Error: {:#?}", e.to_string());
                    };
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });
    }
    Ok(())
}

// Main Request Response Thread
pub async fn spawn_main_loop(
    reject_list: Arc<Mutex<Vec<String>>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let listener = UnixListener::bind(SOCKET_PATH).unwrap();
    loop {
        let reject_list = Arc::clone(&reject_list);
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
                        let cmd: Command =
                            match bincode::decode_from_slice(&buf[..n], config::standard()) {
                                Ok((c, _)) => c,
                                Err(e) => {
                                    eprintln!("Unable to parse Command. Skipping.\n{}", e);
                                    continue;
                                }
                            };
                        let reject_list = Arc::clone(&reject_list);
                        let response = match execute(&cmd, reject_list) {
                            Ok(s) => s,
                            Err(e) => Response::Error(e.to_string()),
                        };
                        if let Response::Error(e) = response.clone() {
                            eprintln!("Response Err: {}", e);
                        }
                        let serialized = bincode::encode_to_vec(&response, config::standard())
                            .expect("Cannot Encode Response.");
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

pub async fn beacond() -> Result<(), Box<dyn Error>> {
    println!("Server Started.\nDaemon listening on {}", SOCKET_PATH);
    let reject_list = Arc::new(Mutex::new(Vec::<String>::new()));
    let reject_list_clone = Arc::clone(&reject_list);

    spawn_ethernet_connection()?;
    spawn_residue_connection()?;
    if let Err(e) = spawn_autoconnection(reject_list_clone).await {
        println!("Error in Autoconnect: {}", e);
    };
    if let Err(e) = spawn_main_loop(Arc::clone(&reject_list)).await {
        println!("Error in Main Loop: {}", e);
    }
    Ok(())
}
