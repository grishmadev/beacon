use std::{
    error::Error,
    io::{Read, Write},
    os::unix::net::UnixStream,
    sync::{Arc, Mutex},
};

use bincode::config;

use crate::{
    Command, Response, SOCKET_PATH,
    backend::functions::{
        connect_to, disconnect_connection, list_active_signals, list_all_signals,
    },
    wifi::helper::{get_current, get_family_info, get_interfaces},
};
pub fn execute(
    cmd: &Command,
    reject_list: Arc<Mutex<Vec<String>>>,
) -> Result<Response, Box<dyn Error + Send + Sync>> {
    let response = match cmd {
        Command::Ping => Response::Pong,

        Command::ListConnections => {
            let hosts =
                list_all_signals().map_err(|e| format!("Failed to list connections: {}", e))?;
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
            let interfaces =
                get_interfaces().map_err(|e| format!("Failed to list interfaces: {}", e))?;
            Response::AllInterfaces(interfaces)
        }

        Command::Notification(msg) => Response::Notification(msg.to_owned()),
        Command::ClearNotification => Response::ClearNotification,
        Command::Connect {
            host,
            iface,
            password,
        } => {
            let list = Arc::clone(&reject_list);
            match connect_to(iface, host.clone(), password, Some(list)) {
                Ok(_) => Response::Connected,
                Err(e) => Response::Error(format!("Could\'nt Connect: {}", e)),
            }
        }
        Command::CurrentConnection => match get_current() {
            Ok(curcon) => Response::CurrentConnection(curcon),
            Err(err) => Response::Error(err.to_string()),
        },

        Command::Disconnect(ifname) => {
            let list = Arc::clone(&reject_list);
            match disconnect_connection(ifname, Some(list)) {
                Ok(_) => Response::Disconnected,
                Err(e) => Response::Error(format!("Couldn't Disconnect. {}", e)),
            }
        }

        Command::Tick => Response::Tick,
        _ => Response::Error("Unknown Command.".into()),
    };
    Ok(response)
}

pub async fn response(cmd: &Command) -> Result<Response, Box<dyn Error>> {
    let mut socket = UnixStream::connect(SOCKET_PATH)?;
    let serialized = bincode::encode_to_vec(cmd, config::standard())?;
    socket.write_all(&serialized)?;
    socket.shutdown(std::net::Shutdown::Write)?;

    let mut buffer = Vec::new();
    let size = socket.read_to_end(&mut buffer)?;

    let (deserialized, _) = bincode::decode_from_slice(&buffer[..size], config::standard())?;
    Ok(deserialized)
}
