use beacon::{
    Command, Response, backend::functions::list_interfaces, executer::execute, mac_to_bytes,
    types::InterfaceType, wifi::wpa_supplicant::connect_via_ethernet,
};
use std::{
    error::Error,
    fs,
    thread::{self, sleep},
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

    let listener = UnixListener::bind(SOCKET_PATH)?;
    println!("Daemon listening on {}", SOCKET_PATH);

    let mut connected_via_ether = false;
    thread::spawn(move || {
        loop {
            let ifaces = list_interfaces().unwrap();
            if !connected_via_ether
                // && let Ok(ifaces) = list_interfaces()
                && let Some(iface) = ifaces
                    .iter()
                    .find(|iface| iface.iftype == InterfaceType::Wired)
                && let Some(ifindex) = iface.ifindex
                && let Some(ifname) = iface.ifname.clone()
                && let Some(mac) = iface.mac.clone()
                && connect_via_ethernet(ifindex, &ifname, mac_to_bytes(&mac)).is_ok()
            {
                println!("Connected via Ethernet");
                connected_via_ether = true;
            } else if connected_via_ether
                // && let Ok(ifaces) = list_interfaces()
                && let None = ifaces.iter().find(|f| f.iftype == InterfaceType::Wired)
            {
                connected_via_ether = false;
                println!("Disconnected Ethernet");
                thread::sleep(Duration::from_millis(800));
            }
        }
    });
    loop {
        let (mut socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            let n = socket.read(&mut buf).await.unwrap();

            let cmd: Command = bincode::deserialize(&buf[..n]).unwrap();
            // dwrite(format!("Command recieved: {:#?}", cmd));
            println!("Command recieved: {:#?}", cmd);
            let response = match execute(&cmd).await {
                Ok(s) => s,
                Err(e) => Response::Error(e.to_string()),
            };
            // dwrite(format!("Response: {:#?}", response));
            println!("Response: {:#?}", response);
            let serialized = bincode::serialize(&response).unwrap();
            socket
                .write_all(&serialized)
                .await
                .expect("Couldn't Write to File");
        });
    }
}
