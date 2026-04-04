use crate::wifi::scan_wifi_networks;

mod wifi;
fn main() {
    println!("Hello Rust!");
    let _ = scan_wifi_networks();
}
