use beacon::{
    Command, Response, backend::functions::list_interfaces, executer::execute, mac_to_bytes,
    types::InterfaceType, wifi::wpa_supplicant::connect_via_ethernet,
};
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
    thread::spawn(move || {
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
            thread::sleep(Duration::from_secs(2));
        }
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
            match socket.read(&mut buf).await {
                Ok(0) => {}
                Ok(n) => {
                    let cmd: Command = bincode::deserialize(&buf[..n]).unwrap();
                    // println!("Command: {:#?}", cmd);
                    let response = match execute(&cmd).await {
                        Ok(s) => s,
                        Err(e) => Response::Error(e.to_string()),
                    };
                    // println!("Response: {:#?}", response);
                    let serialized = bincode::serialize(&response).unwrap();
                    socket
                        .write_all(&serialized)
                        .await
                        .expect("Couldn't Write to File");
                }
                Err(e) => {
                    eprint!("Socket Error: {}", e);
                }
            };
        });
    }
}
