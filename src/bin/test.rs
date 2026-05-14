use beacon::{
    Command, Response,
    backend::functions::{disconnect_connection, list_active_signals, list_interfaces},
    executer::execute,
    mac_to_bytes,
    wifi::{
        helper::{get_current_ip, get_family_info, get_gateway_ip, get_scan, trigger_scan},
        wpa_supplicant::{connect_via_ethernet, request_host},
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
                .starts_with("en")
        })
        .unwrap();
    let ifindex = interface.ifindex.unwrap();
    let ifname = interface.ifname.clone().unwrap();
    let family_info = get_family_info().unwrap();
    // let cmd = Command::ListActiveConnections(interface.clone());
    // let response = execute(&cmd).await.unwrap();
    let mac = mac_to_bytes(&interface.mac.clone().unwrap());
    let current_ip = get_current_ip().unwrap().unwrap();
    let server_id = get_gateway_ip();
    // let server_ip = get
    println!("current_ip: {:#?}", current_ip);

    let hosts = list_active_signals(&family_info, interface.clone());
    // println!("hosts: {:#?}", hosts);
    // loop {
    // match request_host(mac, current_ip, get_gateway_ip().unwrap(), true) {
    //     Ok(s) => {
    //         println!("response: {:#?}", s);
    //         // break;
    //     }
    //     Err(e) => {
    //         println!("Error: {:#?}", e);
    //         // continue;
    //         // break;
    //     } // };
    // }
    connect_via_ethernet(ifindex, &ifname);
}
