use beacon::{
    Command,
    backend::functions::{list_active_signals, list_interfaces},
    executer::response,
    wifi::{
        helper::{
            get_current_ip, get_family_info, get_gateway_ip, remove_lease_and_gateway_ip,
            return_on_disconnect,
        },
        wpa_supplicant::disconnect,
    },
};

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
    let server_id = get_gateway_ip();
    println!("current_ip: {:#?}", current_ip);
    // let res = connect(interface, "刀", "kakakakaka").await;
    // match return_on_disconnect(ifindex as i32) {
    //     Ok(_) => {
    //         if let Err(e) = remove_lease_and_gateway_ip(ifindex, current_ip, server_id.unwrap(), 64)
    //         {
    //             eprintln!("test Error: {}", e);
    //         }
    //         println!("Disconnected.")
    //     }
    //
    //     Err(e) => {
    //         println!("Err: {}", e);
    //     }
    // };
    // let res = response(&Command::ListActiveConnections(interface.clone())).await;
    // println!("Response: {:#?}", res);
    let res = list_active_signals(&family_info, interface.clone());
    println!("Hosts: {:#?}", res);
}
