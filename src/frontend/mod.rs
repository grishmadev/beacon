use beacon::{Command, Response, SOCKET_PATH};

pub fn command(cmd: Command) -> Result<Response, Box<dyn Error>> {
    let mut socket = UnixStream::connect(SOCKET_PATH)?;

    println!("Command sent: {:?}", cmd);
    let serialized = bincode::serialize(&cmd)?;
    socket.write_all(&serialized)?;

    let mut buf = [0; 1024];
    let n = socket.read(&mut buf)?;
    let response: Response = bincode::deserialize(&buf[..n])?;

    println!("Response: {:#?}", response);
    Ok(response)
}
