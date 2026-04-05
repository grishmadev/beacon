use beacon::wifi::{self, helper};
fn main() {
    println!("Hello Rust!");
    let _ = wifi::scan_wifi_networks();
    let family_info = helper::get_family_info().unwrap();
    let current_connection = helper::get_current(family_info.id).unwrap();
    println!("current_connection: {:#?}", current_connection);
}
