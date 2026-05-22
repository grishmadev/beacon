use beacon::{
    DAEMON_ERR_PATH, DAEMON_OUT_PATH,
    backend::threads::{
        beacond, spawn_ethernet_connection, spawn_main_loop, spawn_residue_connection,
    },
};
use clap::Parser;
use daemonize::Daemonize;
use std::{
    error::Error,
    fs::{self, File},
};
use tokio::runtime::Runtime;

use beacon::SOCKET_PATH;

#[derive(Parser, Debug)]
#[command(
    version = "0.1.0",
    about = "Beacon\nLightweight Wifi Manager made in Rust."
)]
struct Args {
    /// Daemonize the Process
    #[arg(short = 'b', long = "background")]
    background: bool,
}
fn main() -> Result<(), Box<dyn Error>> {
    // Clean up old socket file if it exists
    let args = Args::parse();
    let _ = fs::remove_file(SOCKET_PATH);
    if args.background {
        let stderr = File::create(DAEMON_ERR_PATH)?;
        let stdout = File::create(DAEMON_OUT_PATH)?;

        let daemonize = Daemonize::new()
            .pid_file("/tmp/beacon.pid")
            .chown_pid_file(true)
            .working_directory("/tmp")
            .stdout(stdout)
            .stderr(stderr);

        match daemonize.start() {
            Ok(_) => {
                println!("Beacon running in background.");
            }
            Err(e) => {
                eprintln!("Error: {:#?}", e);
                std::process::exit(1);
            }
        }
    }
    let runtime = Runtime::new()?;

    runtime.block_on(async {
        if let Err(e) = beacond().await {
            eprintln!("Beacon Crashed: {}", e);
        }
    });

    Ok(())
}
