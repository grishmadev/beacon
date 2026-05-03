use beacon::{
    Command,
    backend::{executer::execute, functions::list_interfaces},
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
    println!("interfaces: {:?}", interface);
    let cmd = Command::ListActiveConnections(interface.clone());
    let response = execute(&cmd).await;
    println!("response: {:#?}", response);
}
