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
    wifi::{
        helper::{get_current, get_family_info, get_interfaces},
        wpa_supplicant::{find_active_interface, request_host_data},
    },
};

pub async fn execute(cmd: &Command) -> Result<Response, Box<dyn Error>> {
    let family_info = get_family_info()?;
    // let family_id = family_info.id;
    let interfaces = get_interfaces()?;
    let active_interface = find_active_interface(&interfaces);
    let active_ifname = active_interface?.unwrap().ifname.to_owned().unwrap();
    let response: Response = match cmd {
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

        Command::ListInterfaces => Response::AllInterfaces(interfaces),

        Command::Notification(msg) => Response::Notification(msg.clone()),

        Command::Connect {
            bssid,
            iface,
            password,
        } => {
            connect_to(&family_info, &interfaces, iface, bssid, password).await?;
            Response::Connected
        }
        Command::CurrentConnection => match current_connection() {
            Ok(curcon) => Response::CurrentConnection(curcon),
            Err(err) => Response::Error(err.to_string()),
        },

        Command::Disconnect => {
            disconnect_connection(&active_ifname)?;
            Response::Ok
        }

        Command::Tick => Response::Tick,
        _ => Response::Error("Unknown Command.".into()),
    };
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
