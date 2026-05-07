use beacon::{
    Command, Response,
    backend::functions::{disconnect_connection, list_interfaces},
    executer::execute,
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
    let cmd = Command::ListActiveConnections(interface.clone());
    let response = execute(&cmd).await.unwrap();
    if let Response::ActiveHosts(ifname, hosts) = response {
        println!("response: {:?} {:#?}", ifname, hosts);
        // let host = hosts[0].clone();
        // let connect = Command::Connect {
        //     bssid: host.clone().bssid.unwrap(),
        //     password: Some("123456890".to_string()),
        //     iface: interface.clone(),
        // };
        // match execute(&connect).await {
        //     Ok(_) => print!("Connected"),
        //     Err(e) => println!("Error: {}", e),
        // };
    }
    disconnect_connection("wlo1");
}
