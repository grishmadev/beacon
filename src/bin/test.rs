use beacon::{
    backend::functions::{list_active_signals, list_interfaces},
    wifi::helper::{get_current_ip, get_family_info, get_gateway_ip},
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
    let family_info = get_family_info().unwrap();
    let _current_ip = get_current_ip(None).unwrap().unwrap();
    let _server_id = get_gateway_ip();
    let res = list_active_signals(&family_info, interface.clone());
    println!("Hosts: {:#?}", res);
}
