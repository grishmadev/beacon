use std::{
    error::Error,
    io::{Read, Write},
    os::unix::net::UnixStream,
};

use crate::{
    Command, Response, SOCKET_PATH,
    backend::functions::{
        connect_to, current_connection, disconnect_connection, list_active_signals,
        list_all_signals,
    },
    wifi::helper::{autoconnect, get_family_info, get_interfaces},
};

const RETRIES: u32 = 5;

pub async fn execute(
    cmd: &Command,
    reject_list: &mut Vec<String>,
) -> Result<Response, Box<dyn Error>> {
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
                    let connections_clone = connections.clone();
                    let iface_clone = iface.clone();
                    let _ = autoconnect(connections_clone, &iface_clone, reject_list);
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
                host,
                iface,
                password,
            } => match connect_to(iface, host.clone(), password, reject_list).await {
                Ok(_) => Response::Connected,
                Err(e) => Response::Error(format!("Could\'nt Connect: {}", e)),
            },
            Command::CurrentConnection => match current_connection() {
                Ok(curcon) => Response::CurrentConnection(curcon),
                Err(err) => Response::Error(err.to_string()),
            },

            Command::Disconnect(ifname) => match disconnect_connection(ifname, reject_list) {
                Ok(_) => Response::Disconnected,
                Err(e) => Response::Error(format!("Couldn't Disconnect. {}", e)),
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
