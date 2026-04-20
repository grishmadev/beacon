use std::error::Error;

use crate::{
    Command, Response,
    functions::{list_active_signals, list_signals},
};

pub fn execute(cmd: &Command) -> Result<Response, Box<dyn Error>> {
    let response: Response = match cmd {
        Command::Ping => Response::Pong,

        Command::List => {
            let hosts = list_signals()?;
            Response::SavedConnections(hosts)
        }

        Command::ListActive => {
            let connections = list_active_signals()?;
            Response::ActiveHosts(connections)
        }

        _ => Response::Ok,
    };
    Ok(response)
}
