use beacon::wifi::helper::{get_current_ip, get_gateway_ip};

#[tokio::main]
async fn main() {
    let ip = get_current_ip(None);
    println!("get_current_ip(None): {:?}", ip);
    let gw = get_gateway_ip();
    println!("get_gateway_ip(): {:?}", gw);
}
