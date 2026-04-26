use beacon::{
    Command, Response,
    backend::{executer::execute, functions::list_active_signals},
    wifi::helper::{get_family_info, get_interfaces},
};
use std::{
    error::Error,
    fs,
    io::{Read, Write},
    os::unix::net::UnixListener,
};

use beacon::SOCKET_PATH;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Clean up old socket file if it exists
    let _ = fs::remove_file(SOCKET_PATH);

    let listener = UnixListener::bind(SOCKET_PATH)?;
    println!("Daemon listening on {}", SOCKET_PATH);
    // let family_info = get_family_info()?;
    // let interfaces = get_interfaces()?;

    // let response = list_active_signals(&family_info, &interfaces).expect("error in response..");
    // println!("Response : {:?}", response);
    loop {
        let (mut socket, _) = listener.accept()?;

        tokio::spawn(async move {
            let mut buf = [0; 1024];
            let n = socket.read(&mut buf).unwrap();

            let cmd: Command = bincode::deserialize(&buf[..n]).unwrap();
            println!("COmmand recieved: {:?}", cmd);

            let response = match execute(&cmd).await {
                Ok(s) => s,
                Err(e) => Response::Error(e.to_string()),
            };
            let serialized = bincode::serialize(&response).unwrap();
            socket.write_all(&serialized).unwrap();
        });
    }
}

// async fn main() -> Result<(), Box<dyn Error>> {
//     // let status = Command::new("ip")
//     //     .args(["link", "set", "wlo1", "up"])
//     //     .status()?;
//     //
//     // if !status.success() {
//     //     eprintln!("failed to bring wlo1 up");
//     // }
//     // let status = Command::new("wpa_supplicant")
//     //     .args(["-B", "-iwlo1", "-c/etc/wpa_supplicant/wpa_supplicant.conf"])
//     //     .status()?;
//     // if !status.success() {
//     //     eprintln!("failed to start wpa_supplicant");
//     //     return Ok(());
//     // }
//
//     let current_connection = wifi::get_current_connection()?;
//     println!("current_connection: {:#?}", current_connection);
//     let hosts = wifi::get_active_networks()?;
//     display_hosts(hosts.clone());
//     // let host = hosts[0].clone();
//     // let ssid = host.ssid.unwrap();
//     // name of my ssid
//     let dao_bytes: &[u8] = b"\xE5\x88\x80";
//     let ssid = std::str::from_utf8(dao_bytes).unwrap();
//     println!("ssid: {:02X?}", ssid.as_bytes());
//     let family_info = wifi::helper::get_family_info()?;
//     let iface = get_interface(family_info.id)?[0].clone();
//     let ifname = iface.ifname.unwrap();
//     let mac = mac_to_bytes(&iface.mac.unwrap());
//     wifi::wpa_supplicant::connect(
//         mac,
//         &ifname,
//         &iface.ifindex.unwrap(),
//         ssid,
//         Some("kakakakaka"),
//     )
//     .await?;
//     Ok(())
// }
