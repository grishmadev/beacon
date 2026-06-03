use std::{
    error::Error,
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
};

use chrono::Utc;

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
    let time = Utc::now().time();
    let msg = match msg_type {
        Log::Ok => "OK",
        Log::Err => "Error",
        Log::Info => "Info",
        Log::Warn => "Warn",
    };
    eprintln!("[ {time} ][ {msg} ] {text}");
}
