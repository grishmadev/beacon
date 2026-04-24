use std::error::Error;

use crate::{
    Command, Response,
    functions::{connect_to, disconnect_connection, list_active_signals, list_all_signals},
    wifi::{
        helper::{get_family_info, get_interfaces},
        wpa_supplicant::find_active_interface,
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

        Command::ListActiveConnections => {
            let connections = list_active_signals(&family_info, &interfaces)?;
            Response::ActiveHosts(connections)
        }

        Command::ListInterfaces => Response::AllInterfaces(interfaces),

        Command::Connect {
            bssid,
            iface,
            password,
        } => {
            connect_to(&family_info, &interfaces, iface, bssid, password).await?;
            Response::Ok
        }

        Command::Disconnect => {
            disconnect_connection(&active_ifname)?;
            Response::Ok
        }

        _ => Response::Error("Unknown Command.".into()),
    };
    Ok(response)
}
