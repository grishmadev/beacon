use std::{
    error::Error,
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
};

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
