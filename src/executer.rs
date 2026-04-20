use std::error::Error;

use crate::{
    Command, Response,
    functions::{connect_to, list_active_signals, list_all_signals},
    wifi::helper::{get_family_info, get_interface},
};

pub async fn execute(cmd: &Command) -> Result<Response, Box<dyn Error>> {
    let family_info = get_family_info()?;
    let family_id = family_info.id;
    let interfaces = get_interface(family_id)?;
    let response: Response = match cmd {
        Command::Ping => Response::Pong,

        Command::List => {
            let hosts = list_all_signals()?;
            Response::SavedConnections(hosts)
        }

        Command::ListActive => {
            let connections = list_active_signals(&family_info, interfaces)?;
            Response::ActiveHosts(connections)
        }

        Command::Connect {
            bssid,
            iface,
            password,
        } => {
            connect_to(&family_info, interfaces, iface, bssid, password).await?;
            Response::Ok
        }

        _ => Response::Ok,
    };
    Ok(response)
}
