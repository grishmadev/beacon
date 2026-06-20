use beacon::wifi::helper::get_current;

#[tokio::main]
async fn main() {
    let cur = get_current();
    println!("current: {:#?}", cur);
}
