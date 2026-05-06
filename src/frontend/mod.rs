use crate::{Command, Response, SOCKET_PATH, debug::write};
pub mod app;
pub mod ui;
use std::{
    error::Error,
    io::{Read, Write},
    os::unix::net::UnixStream,
};

pub fn command(cmd: Command) -> Result<Response, Box<dyn Error>> {
    let mut socket = UnixStream::connect(SOCKET_PATH)?;

    write(format!("Command sent: {:?}", cmd));
    let serialized = bincode::serialize(&cmd)?;
    socket.write_all(&serialized)?;

    let mut buf = [0; 1024];
    let n = socket.read(&mut buf)?;
    let response: Response = bincode::deserialize(&buf[..n])?;

    write(format!("Response: {:#?}", response));
    Ok(response)
}

// pub fn handle_keys(cmd)
