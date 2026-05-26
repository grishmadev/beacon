mod cmd;
use cmd::*;
mod attr;
use attr::*;
mod parse_attr;
use parse_attr::*;
mod socket;
use socket::*;
mod consts;
use consts::*;
mod interface;
use interface::*;
mod station;
use station::*;
mod bss;
use bss::*;
mod nl80211traits;
use nl80211traits::*;

fn main() -> Result<(), neli::err::NlError>{
    // let interfaces = Socket::connect()?.get_interfaces_info()?;
    // for interface in interfaces {
    //     println!("{}", interface.pretty_format());
    //     // if let Some(index) = interface.index {
    //     //     println!("{:?}", station?.pretty_format());
    //     //     let station = Socket::connect()?.get_bss_info(&index);
    //     // }
    // }
    let interfaces = Socket::connect()?.get_interfaces_info()?;
    for interface in interfaces {
        let station = interface.get_station_info();
        println!("{}", station?.pretty_format());
    }
    Ok(())
}
