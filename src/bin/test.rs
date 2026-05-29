use beacon::{backend::functions::list_interfaces, wifi::wpa_supplicant::discover_host};

#[tokio::main]
async fn main() {
    let iface = list_interfaces()
        .unwrap()
        .iter()
        .find(|i| i.ifname == Some("wlo1".to_string()))
        .unwrap()
        .to_owned();

    let discover = discover_host(&iface).unwrap();
    println!("Discover: {:#?}", discover.ip_addr);
}
