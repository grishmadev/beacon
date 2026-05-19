use std::{
    error::Error,
    io::{Read, Write},
    ops::Mul,
    os::unix::net::UnixStream,
};

use chrono::{TimeZone, Utc};

use crate::{
    Command, Response, SOCKET_PATH,
    backend::functions::{
        connect_to, current_connection, disconnect_connection, list_active_signals,
        list_all_signals,
    },
    types::{Interface, InterfaceType},
    wifi::{
        dhcp_connection::{DhcpFile, DhcpStorage},
        helper::{get_family_info, get_interfaces, renew_connection},
        wpa_supplicant::find_active_interface,
    },
};

const RETRIES: u32 = 5;

pub async fn execute(cmd: &Command) -> Result<Response, Box<dyn Error>> {
    let mut response = Response::Error("Uninitialized Response".into());

    for _ in 0..RETRIES {
        response = match cmd {
            Command::Ping => Response::Pong,

            Command::ListConnections => {
                let hosts = list_all_signals()?;
                Response::SavedHosts(hosts)
            }

            Command::ListActiveConnections(iface) => {
                let family_info = get_family_info()?;
                let connections = list_active_signals(&family_info, iface.clone())?;
                if let Some(ifname) = iface.ifname.clone() {
                    Response::ActiveHosts(ifname, connections)
                } else {
                    Response::Error("Unknown Interface.".into())
                }
            }

            Command::ListInterfaces => {
                let interfaces = get_interfaces()?;
                Response::AllInterfaces(interfaces.clone())
            }

            Command::Notification(msg) => Response::Notification(msg.to_owned()),
            Command::ClearNotification => Response::ClearNotification,
            Command::Connect {
                bssid,
                iface,
                password,
            } => {
                let interfaces = get_interfaces()?;
                let family_info = get_family_info()?;
                match connect_to(&family_info, &interfaces, iface, bssid, password).await {
                    Ok(_) => {
                        manage_lease_thread(iface)?;
                        Response::Connected
                    }
                    Err(e) => Response::Error(format!("Could\'nt Connect: {}", e)),
                }
            }
            Command::CurrentConnection => match current_connection() {
                Ok(curcon) => Response::CurrentConnection(curcon),
                Err(err) => Response::Error(err.to_string()),
            },

            Command::Disconnect => {
                let active_interface = find_active_interface();
                let active_ifname = active_interface?.unwrap().ifname.to_owned().unwrap();

                match disconnect_connection(&active_ifname) {
                    Ok(_) => Response::Disconnected,
                    Err(e) => Response::Error(format!("Couldn't Disconnect. {}", e)),
                }
            }

            Command::Tick => Response::Tick,
            _ => Response::Error("Unknown Command.".into()),
        };
        if let Response::Error(e) = response.clone() {
            println!("Command not Implemented. {}", e);
            continue;
        } else {
            break;
        }
    }
    Ok(response)
}

pub fn manage_lease_thread(iface: &Interface) -> Result<(), Box<dyn Error>> {
    let iface = iface.clone();
    tokio::spawn(async move {
        let mut last_read = DhcpFile::default();
        loop {
            let info = DhcpStorage::read_file();
            if let Ok(files) = info {
                if files.is_empty() {
                    continue;
                }
                if let Some(content) = files.first() {
                    if last_read != *content {
                        last_read = content.clone();
                        println!("New DHCP Connextion: {:#?}", content);
                    }
                    let time_init = content.time_initiated;
                    let ls_dur = content.lease_duration as i64;
                    manage_lease(&iface, time_init, ls_dur);
                }
            } else {
                break;
            }
        }
    });

    Ok(())
}

fn manage_lease(iface: &Interface, time_init: i64, ls_dur: i64) {
    let now = Utc::now();
    let t1 = ls_dur / 2;
    let t2 = ls_dur as f64 * 0.875;
    let time_left = Utc.timestamp_opt(ls_dur + time_init, 0).single().unwrap() - now;
    let time_left = time_left.num_seconds();
    let data = if time_left + t2 as i64 <= ls_dur {
        renew_connection(iface, true)
    } else if time_left + t1 <= ls_dur {
        renew_connection(iface, false)
    } else {
        Err("Nothing happened.".into())
    };
    if let Ok(Some(data)) = data {
        let _ = DhcpStorage::write_from_dhcplease(&data);
    };
}

pub async fn response(cmd: &Command) -> Result<Response, Box<dyn Error>> {
    let mut socket = UnixStream::connect(SOCKET_PATH)?;
    let serialized = bincode::serialize(&cmd)?;
    socket.write_all(&serialized)?;
    socket.shutdown(std::net::Shutdown::Write)?;

    let mut buffer = Vec::new();
    let size = socket.read_to_end(&mut buffer)?;

    let deserialzed: Response = bincode::deserialize(&buffer[..size])?;
    Ok(deserialzed)
}
