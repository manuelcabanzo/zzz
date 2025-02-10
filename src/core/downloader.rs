use std::fs::File;
use std::io::Write;
use std::path::Path;
use reqwest::blocking::get;

pub struct Downloader;

impl Downloader {
    pub fn download_file(url: &str, destination: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let response = get(url)?;
        let mut file = File::create(destination)?;
        file.write_all(&response.bytes()?)?;
        Ok(())
    }
}
