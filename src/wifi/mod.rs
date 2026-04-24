pub mod helper;
pub mod history;
pub mod wpa_supplicant;

// pub fn get_active_networks() -> Result<Vec<Host>, Box<dyn Error>> {
//     let family_info = get_family_info()?;
//     println!("{:#?}", family_info);
//     let interfaces = get_interfaces()?;
//     let mut result = Vec::<Host>::new();
//     for iface in interfaces {
//         let ifname = iface.ifname.unwrap_or("---".to_string());
//         if let Some(ifindex) = iface.ifindex {
//             println!("Scanning {}", ifname);
//             trigger_scan(&family_info, ifindex)?;
//             let hosts = get_scan(family_info.id, ifindex)?;
//             result.extend(hosts);
//         } else {
//             println!("Couldn't Scan {}", ifname);
//         }
//     }
//     Ok(result)
// }

// pub fn display_hosts(hosts: Vec<Host>) {
//     println!(
//         "{:<30} {:<20} {:>10} {:>12}",
//         "SSID", "BSSID", "Frequency(MHz)", "Signal(dBm)"
//     );
//     println!("{}", "=".repeat(100));
//
//     for host in hosts {
//         let ssid = host.ssid.unwrap_or("---".to_string());
//         let bssid = host.bssid.unwrap_or("---".to_string());
//         let frequency = match host.frequency {
//             Some(s) => s.to_string(),
//             None => "-".to_string(),
//         };
//         let signal = match host.signal {
//             Some(s) => s.to_string(),
//             None => "-".to_string(),
//         };
//         println!(
//             "{:<30} {:<20} {:>10} {:>12}",
//             ssid, bssid, frequency, signal
//         );
//         println!("{}", "-".repeat(100));
//     }
// }

// fn display_interfaces(interfaces: &[Interface]) {
//     println!("{:<20} {:<20} {:<30}", "Index", "Name", "Mac");
//     println!("{}", "=".repeat(100));
//     for iface in interfaces {
//         let ifindex = match iface.ifindex {
//             Some(s) => s.to_string(),
//             None => "---".to_string(),
//         };
//         let ifname = iface.ifname.clone().unwrap_or("---".to_string());
//         let mac = iface.mac.clone().unwrap_or("---".to_string());
//         println!("{:<20} {:<20} {:<30}", ifindex, ifname, mac);
//         println!("{}", "-".repeat(100));
//     }
// }
