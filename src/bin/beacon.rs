use std::{
    error::Error,
    io::{Read, Write},
    os::unix::net::UnixStream,
};

use beacon::{Command, Response, SOCKET_PATH};

fn main() -> Result<(), Box<dyn Error>> {
    let mut socket = UnixStream::connect(SOCKET_PATH)?;

    let cmd = Command::ListActiveConnections;
    println!("Command sent: {:?}", cmd);
    let serialized = bincode::serialize(&cmd)?;
    socket.write_all(&serialized)?;

    let mut buf = [0; 1024];
    let n = socket.read(&mut buf)?;
    let response: Response = bincode::deserialize(&buf[..n])?;

    println!("Response: {:#?}", response);

    // let family_info = get_family_info()?;
    // let family_id = family_info.id;
    // let interfaces = list_interfaces(family_id)?;
    // println!("interfaces: {:#?}", interfaces);
    // let active = find_active_interface(&interfaces)?;
    // println!("active interface: {:#?}", active);
    Ok(())
}
