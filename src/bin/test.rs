use beacon::{
    Command, Response,
    backend::functions::{disconnect_connection, list_active_signals, list_interfaces},
    executer::execute,
    mac_to_bytes,
    wifi::{
        dhcp_connection::DhcpStorage,
        helper::{get_current_ip, get_family_info, get_gateway_ip, get_scan, trigger_scan},
        wpa_supplicant::{connect, connect_via_ethernet},
    },
};
use chrono::{TimeZone, Utc};

#[tokio::main]
async fn main() {
    let interfaces = list_interfaces().unwrap();

    let interface = interfaces
        .iter()
        .find(|iface| {
            iface
                .ifname
                .as_ref()
                .unwrap_or(&"---".to_string())
                .starts_with("wlo1")
        })
        .unwrap();
    let ifindex = interface.ifindex.unwrap();
    let ifname = interface.ifname.clone().unwrap();
    let family_info = get_family_info().unwrap();
    // let cmd = Command::ListActiveConnections(interface.clone());
    // let response = execute(&cmd).await.unwrap();
    // let mac = mac_to_bytes(&interface.mac.clone().unwrap());
    let current_ip = get_current_ip().unwrap().unwrap();
    // let server_id = get_gateway_ip();
    println!("current_ip: {:#?}", current_ip);
    // let res = connect_via_ethernet(ifindex, &ifname, mac);
    // println!("Ether: {:#?}", res);
    let res = connect(interface, "刀", "kakakakaka").await;
}
