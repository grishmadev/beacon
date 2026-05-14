use std::{
    collections::HashMap,
    error::Error,
    io::{Read, Write},
    ops::{Add, AddAssign, Mul},
    os::unix::net::UnixStream,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use chrono::{TimeZone, Utc};

use crate::{
    Command, Response, SOCKET_PATH,
    backend::functions::{
        connect_to, current_connection, disconnect_connection, list_active_signals,
        list_all_signals,
    },
    debug::write,
    types::DhcpLease,
    wifi::{
        dhcp_connection::{DhcpFile, DhcpStorage},
        helper::{get_family_info, get_interfaces, renew_connection},
        wpa_supplicant::find_active_interface,
    },
};

const RETRIES: u32 = 5;

struct TimeTracker {
    stop_signal: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl TimeTracker {
    fn stop(&mut self) {
        self.stop_signal.store(false, Ordering::Relaxed);

        if let Some(h) = self.thread.take() {
            h.join().expect("Thread Panicked.");
        }
    }
}

pub async fn execute(cmd: &Command) -> Result<Response, Box<dyn Error>> {
    let family_info = get_family_info()?;
    // let family_id = family_info.id;
    let interfaces = get_interfaces()?;
    let active_interface = find_active_interface();
    let active_ifname = active_interface?.unwrap().ifname.to_owned().unwrap();
    let mut response = Response::Error("Uninitialized Response".into());

    for _ in 0..RETRIES {
        response = match cmd {
            Command::Ping => Response::Pong,

            Command::ListConnections => {
                let hosts = list_all_signals()?;
                Response::SavedHosts(hosts)
            }

            Command::ListActiveConnections(iface) => {
                let connections = list_active_signals(&family_info, iface.clone())?;
                if let Some(ifname) = iface.ifname.clone() {
                    Response::ActiveHosts(ifname, connections)
                } else {
                    Response::Error("Unknown Interface.".into())
                }
            }

            Command::ListInterfaces => Response::AllInterfaces(interfaces.clone()),

            Command::Notification(msg) => Response::Notification(msg.to_owned()),
            Command::ClearNotification => Response::ClearNotification,
            Command::Connect {
                bssid,
                iface,
                password,
            } => match connect_to(&family_info, &interfaces, iface, bssid, password).await {
                Ok(_) => {
                    manage_lease_thread()?;
                    Response::Connected
                }
                Err(e) => Response::Error(format!("Could\'nt Connect: {}", e)),
            },
            Command::CurrentConnection => match current_connection() {
                Ok(curcon) => Response::CurrentConnection(curcon),
                Err(err) => Response::Error(err.to_string()),
            },

            Command::Disconnect => match disconnect_connection(&active_ifname) {
                Ok(_) => Response::Disconnected,
                Err(e) => Response::Error(format!("Couldn't Disconnect. {}", e).into()),
            },

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

pub fn manage_lease_thread() -> Result<(), Box<dyn Error>> {
    thread::spawn(move || {
        loop {
            let info = DhcpStorage::read_file();
            if let Ok(Some(content)) = info {
                let time_init = content.time_initiated;
                let ls_dur = content.lease_duration as i64;
                manage_lease(time_init, ls_dur);
            } else {
                break;
            }
        }
    });

    Ok(())
}

fn manage_lease(time_init: i64, ls_dur: i64) {
    let now = Utc::now();
    let t1 = Utc.timestamp_opt((ls_dur / 2) + time_init, 0).single();
    let t2 = Utc
        .timestamp_opt((ls_dur as f64).mul(0.875) as i64 + time_init, 0)
        .single();
    if let Some(t1) = t1
        && let Some(t2) = t2
    {
        let data = if now > t2 {
            renew_connection(true)
        } else if now > t1 {
            renew_connection(false)
        } else {
            Err("Nothing happened.".into())
        };
        if let Ok(Some(data)) = data {
            DhcpStorage::write_file(&mut DhcpFile {
                ip_addr: data.ip_addr,
                subnet_mask: data.subnet_mask,
                gateway: data.gateway,
                dns_servers: data.dns_servers,
                server_id: data.server_id,
                lease_duration: data.lease_duration,
                ..Default::default()
            });
        };
    } else {
        panic!("Cannot parse time.");
    }
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
