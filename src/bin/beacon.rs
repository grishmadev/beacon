use std::{
    error::Error,
    io::{Read, Write},
    os::unix::net::{UnixListener, UnixStream},
};

use beacon::{Command, Response, SOCKET_PATH};

fn main() -> Result<(), Box<dyn Error>> {
    println!("Hello beacon");
    let mut socket = UnixStream::connect(SOCKET_PATH)?;

    let cmd = Command::Ping;
    let serialized = bincode::serialize(&cmd)?;
    socket.write_all(&serialized)?;

    let mut buf = [0; 1024];
    let n = socket.read(&mut buf)?;
    let response: Response = bincode::deserialize(&buf[..n])?;

    println!("Response: {:?}", response);

    Ok(())
}
