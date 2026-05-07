use beacon::{Command, Response, executer::execute};
use std::{
    error::Error,
    fs,
    io::{Read, Write},
    os::unix::net::UnixListener,
};

use beacon::SOCKET_PATH;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Clean up old socket file if it exists
    let _ = fs::remove_file(SOCKET_PATH);

    let listener = UnixListener::bind(SOCKET_PATH)?;
    println!("Daemon listening on {}", SOCKET_PATH);
    loop {
        let (mut socket, _) = listener.accept()?;

        tokio::spawn(async move {
            let mut buf = [0; 1024];
            let n = socket.read(&mut buf).unwrap();

            let cmd: Command = bincode::deserialize(&buf[..n]).unwrap();
            // dwrite(format!("Command recieved: {:#?}", cmd));
            println!("Command recieved: {:#?}", cmd);
            let response = match execute(&cmd).await {
                Ok(s) => s,
                Err(e) => Response::Error(e.to_string()),
            };
            // dwrite(format!("Response: {:#?}", response));
            println!("Response: {:#?}", response);
            let serialized = bincode::serialize(&response).unwrap();
            socket.write_all(&serialized).unwrap();
        });
    }
}
