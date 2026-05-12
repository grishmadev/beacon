use std::{
    collections::HashMap,
    error::Error,
    io::{Read, Write},
    os::unix::net::UnixStream,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::{
    Command, Response, SOCKET_PATH,
    backend::functions::{
        connect_to, current_connection, disconnect_connection, list_active_signals,
        list_all_signals,
    },
    debug::write,
    wifi::{
        helper::{get_family_info, get_interfaces, renew_connection},
        wpa_supplicant::{find_active_interface, request_host_data},
    },
};
use futures::future;

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
                    let stop_signal = Arc::new(AtomicBool::new(true));
                    let signal_for_thread = Arc::clone(&stop_signal);
                    let ifname = iface.ifname.clone().unwrap();
                    let id =
                        spawn_lease_manager(ifname, Duration::from_secs(3600), signal_for_thread)?;
                    let thread_handle = TimeTracker {
                        thread: Some(id),
                        stop_signal,
                    };

                    Response::Connected
                }
                Err(e) => Response::Error(format!("Could\'nt Connect: {}", e)),
            },
            Command::CurrentConnection => {
                // let mut response: Response = Response::Error("Uninitialized Response".into());
                // let response = tokio::task::spawn_blocking(move || match current_connection() {
                //     Ok(curcon) => Response::CurrentConnection(curcon),
                //     Err(err) => Response::Error(err.to_string()),
                // })
                // .await
                // .unwrap_or(Response::Error("Thread Panicked".into()));
                //
                // response
                match current_connection() {
                    Ok(curcon) => Response::CurrentConnection(curcon),
                    Err(err) => Response::Error(err.to_string()),
                }
            }

            Command::Disconnect => match disconnect_connection(&active_ifname) {
                Ok(_) => Response::Ok,
                Err(e) => Response::Error("Couldn't Disconnect.".into()),
            },

            Command::Tick => Response::Tick,
            _ => Response::Error("Unknown Command.".into()),
        };
        if let Response::Error(_) = response {
            continue;
        } else {
            break;
        }
    }
    Ok(response)
}

pub fn spawn_lease_manager(
    ifname: String,
    lease_duration: Duration,
    stop_signal: Arc<AtomicBool>,
) -> Result<JoinHandle<()>, Box<dyn Error>> {
    let handle = std::thread::spawn(move || {
        let start = Instant::now();
        let t1 = lease_duration.div_f32(2.0);
        let t2 = lease_duration.mul_f32(0.875);

        let mut renewed = false;
        while stop_signal.load(std::sync::atomic::Ordering::Relaxed) {
            let elapsed = start.elapsed();

            if elapsed >= lease_duration {
                let _ = write(format!("[{}] Lease Expired. Dropping IP.", ifname));
                break;
            }
            if elapsed >= t2 {
                renew_connection(true);
            } else if elapsed >= t1 && !renewed {
                if renew_connection(false).is_ok() {
                    renewed = true;
                };
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    });
    Ok(handle)
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
