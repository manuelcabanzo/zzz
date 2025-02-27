use std::path::PathBuf;
use std::fs;
use std::io::Write;
use std::time::Duration;
use reqwest::Client;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::time::timeout;
use std::sync::Arc;
use futures_util::StreamExt;
use bytes::Bytes;

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);
const SDK_BASE_URL: &str = "https://dl.google.com/android/repository";
const PROGRESS_REPORT_THRESHOLD: u64 = 1024 * 1024; // Report every 1MB

pub struct AndroidSdkManager {
    sdk_path: PathBuf,
    client: Client,
}

impl AndroidSdkManager {
    pub fn new() -> Self {
        let app_data = dirs::config_dir()
            .expect("Could not find config directory")
            .join("zzz")
            .join("android-sdk");
            
        println!("SDK path: {}", app_data.display());
        fs::create_dir_all(&app_data).expect("Could not create SDK directory");

        Self {
            sdk_path: app_data,
            client: Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .expect("Could not create HTTP client"),
        }
    }

    pub async fn ensure_api_level(&self, api_level: &str, progress_callback: Arc<dyn Fn(f32) + Send + Sync>) -> Result<(), Box<dyn std::error::Error>> {
        let platform_dir = self.sdk_path.join("platforms").join(format!("android-{}", api_level));
        println!("Platform directory: {}", platform_dir.display());
        
        if platform_dir.exists() {
            println!("Platform already downloaded");
            (progress_callback)(1.0);
            return Ok(());
        }

        fs::create_dir_all(&platform_dir)?;
        
        let filename = format!("platform-{}_r01.zip", api_level);
        let url = format!("{}/{}", SDK_BASE_URL, filename);
        println!("Downloading from URL: {}", url);

        let response = match timeout(DOWNLOAD_TIMEOUT, self.client.get(&url).send()).await {
            Ok(Ok(response)) => {
                if !response.status().is_success() {
                    return Err(format!("Failed to download: HTTP {}", response.status()).into());
                }
                response
            },
            Ok(Err(e)) => return Err(format!("Request error: {}", e).into()),
            Err(_) => return Err("Download timed out".into()),
        };

        let total_size = response.content_length().unwrap_or(0);
        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?);

        println!("Starting download of {} bytes", total_size);
        
        let mut downloaded = 0u64;
        let mut last_reported = 0u64;
        let mut temp_file = tempfile::NamedTempFile::new()?;
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            let chunk: Bytes = chunk_result?;
            temp_file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;
            
            // Update progress bar and callback less frequently
            if downloaded - last_reported >= PROGRESS_REPORT_THRESHOLD {
                pb.set_position(downloaded);
                (progress_callback)(downloaded as f32 / total_size as f32);
                last_reported = downloaded;
                
                // Yield to allow UI to update
                tokio::task::yield_now().await;
            }
        }

        // Final progress update
        pb.set_position(downloaded);
        (progress_callback)(1.0);
        pb.finish_with_message("Download completed");

        println!("Extracting SDK files...");
        let temp_file = temp_file.reopen()?;
        let mut archive = zip::ZipArchive::new(temp_file)?;
        
        // Extract files with periodic yields to prevent UI blocking
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = platform_dir.join(file.mangled_name());
            
            if file.is_dir() {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(parent) = outpath.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut outfile = fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
            
            // Yield periodically during extraction
            if i % 100 == 0 {
                tokio::task::yield_now().await;
            }
        }

        println!("API level {} installation completed", api_level);
        Ok(())
    }

    pub fn get_sdk_path(&self) -> PathBuf {
        self.sdk_path.clone()
    }

    pub fn get_platform_dir(&self, api_level: &str) -> PathBuf {
        self.sdk_path.join("platforms").join(format!("android-{}", api_level))
    }
}
