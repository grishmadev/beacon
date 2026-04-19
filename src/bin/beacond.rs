use std::{error::Error, fs, io::Read, os::unix::net::UnixListener, process::Command};

use beacon::{
    SOCKET_PATH, mac_to_bytes,
    wifi::{self, display_hosts, helper::get_interface},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let _ = fs::remove_file(SOCKET_PATH);

    let listener = UnixListener::bind(SOCKET_PATH)?;
    println!("beacond v0.01 listening on: {}", SOCKET_PATH);

    loop {
        let (mut socket, addr) = listener.accept()?;
        tokio::spawn(async move {
            let mut buf = [0, 1024];
            let n = socket.read(&mut buf)?;

            // Desrialie here
            let cmd: Command = bincode::deserialize(&buf[..n]).unwrap();
            println!("Recieved Command: {}", cmd);
        });
    }

    Ok(())
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
