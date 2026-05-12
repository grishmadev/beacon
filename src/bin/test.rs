use beacon::{
    Command, Response,
    backend::functions::{disconnect_connection, list_interfaces},
    executer::execute,
    mac_to_bytes,
    wifi::{
        helper::{get_current_ip, get_gateway_ip},
        wpa_supplicant::get_current_host_data,
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
                .starts_with("wl")
        })
        .unwrap();
    // let cmd = Command::ListActiveConnections(interface.clone());
    // let response = execute(&cmd).await.unwrap();
    let mac = mac_to_bytes(&interface.mac.clone().unwrap());
    let current_ip = get_current_ip().unwrap().unwrap();
    let server_id = get_gateway_ip();
    // let server_ip = get
    println!("current_ip: {:#?}", current_ip);
    loop {
        match get_current_host_data(mac, current_ip, get_gateway_ip().unwrap()) {
            Ok(s) => {
                println!("response: {:#?}", s);
                break;
            }
            Err(e) => {
                println!("Error: {:#?}", e);
                // continue;
                break;
            }
        };
    }

    // disconnect_connection("wlo1");
}
