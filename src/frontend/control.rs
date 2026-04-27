use std::{
    error::Error,
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use crate::{Command, Response, frontend::command};

pub struct CommandTransmitter {
    pub sx: Sender<Command>,
    pub rx: Receiver<Command>,
}

pub struct ResponseTransmitter {
    pub sx: Sender<Response>,
    pub rx: Receiver<Response>,
}

impl CommandTransmitter {
    pub fn default() -> Self {
        let (sx, rx) = mpsc::channel::<Command>();
        Self { rx, sx }
    }
}

impl ResponseTransmitter {
    pub fn default() -> Self {
        let (sx, rx) = mpsc::channel::<Response>();
        Self { rx, sx }
    }
}
pub fn init_command_transmitter() -> Result<(Sender<Command>, Receiver<Command>), Box<dyn Error>> {
    let cmd_tns = CommandTransmitter::default();

    Ok((cmd_tns.sx, cmd_tns.rx))
}
