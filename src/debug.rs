use std::{
    error::Error,
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
};

use chrono::Local;

use crate::Log;

pub fn write(logs: String) -> Result<(), Box<dyn Error>> {
    let path_str = "./debug.txt";
    let path = Path::new(path_str);
    if !path.exists() {
        File::create(path_str)?;
    };
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(logs.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}
pub fn dwrite(logs: String) -> Result<(), Box<dyn Error>> {
    let path_str = "./logs.txt";
    let path = Path::new(path_str);
    if !path.exists() {
        File::create(path_str)?;
    };
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(logs.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

pub fn log_msg(text: &str, msg_type: Log) {
    let now = Local::now();
    let datetime = now.format("%Y-%m-%d %H:%M:%S");
    let msg = match msg_type {
        Log::Ok => "OK",
        Log::Err => "Error",
        Log::Info => "Info",
        Log::Warn => "Warn",
    };
    eprintln!("[ {datetime} ][ {msg} ] {text}");
}
