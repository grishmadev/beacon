use crate::{Command, Response, SOCKET_PATH, debug::write};
pub mod app;
pub mod ui;
use std::{
    error::Error,
    io::{Read, Write},
    os::unix::net::UnixStream,
};

pub fn sigrate_to_bars(sigrate: i32) -> String {
    let sigrate = -sigrate;
    let bar = if sigrate < 30 {
        "BEST"
    } else if sigrate >= 30 && sigrate <= 50 {
        "||||||||"
    } else if sigrate > 50 && sigrate <= 60 {
        "||||||"
    } else if sigrate > 60 && sigrate <= 67 {
        "||||"
    } else if sigrate > 70 && sigrate <= 80 {
        "|||"
    } else if sigrate > 80 && sigrate <= 90 {
        "||"
    } else {
        "---"
    };
    bar.to_string()
}
