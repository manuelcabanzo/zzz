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
use std::process::Command;

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

    pub fn accept_licenses(&self) -> Result<(), Box<dyn std::error::Error>> {
        let licenses_dir = self.sdk_path.join("licenses");
        fs::create_dir_all(&licenses_dir)?;

        // Write default license files
        let license_files = [
            ("android-sdk-license", "24333f8a63b6825ea9c5514f83c2829b004d1fee"),
            ("android-sdk-preview-license", "84831b9409646a918e30573bab4c9c91346d8abd"),
            ("intel-android-extra-license", "d975f751698a77b662f1254ddbeed3901e976f5a"),
        ];

        for (filename, content) in license_files.iter() {
            let file_path = licenses_dir.join(filename);
            fs::write(file_path, content)?;
        }

        // Also try to accept licenses through sdkmanager if available
        if let Ok(output) = Command::new("sdkmanager")
            .arg("--licenses")
            .current_dir(&self.sdk_path)
            .output() 
        {
            println!("SDKManager license output: {}", String::from_utf8_lossy(&output.stdout));
        }

        Ok(())
    }

    pub async fn ensure_api_level(&self, api_level: &str, progress_callback: Arc<dyn Fn(f32) + Send + Sync>) -> Result<(), Box<dyn std::error::Error>> {
        // Accept licenses first
        self.accept_licenses()?;

        let platform_dir = self.sdk_path.join("platforms").join(format!("android-{}", api_level));
        println!("Platform directory: {}", platform_dir.display());
        
        if platform_dir.exists() {
            println!("Platform already downloaded");
            (progress_callback)(1.0);
            return Ok(());
        }

        fs::create_dir_all(&platform_dir)?;
        
        // Fixed URL format for Android platform downloads
        let url = format!("{}/platforms/android-{}_r02.zip", SDK_BASE_URL, api_level);
        println!("Downloading from URL: {}", url);

        let response = match timeout(DOWNLOAD_TIMEOUT, self.client.get(&url).send()).await {
            Ok(Ok(response)) => {
                if !response.status().is_success() {
                    // Try alternative URL format if first one fails
                    let alt_url = format!("{}/platform-{}_r02.zip", SDK_BASE_URL, api_level);
                    println!("Retrying with alternate URL: {}", alt_url);
                    let alt_response = self.client.get(&alt_url).send().await?;
                    if !alt_response.status().is_success() {
                        // Try a third format as last resort
                        let last_url = format!("{}/android-{}/android-{}.zip", SDK_BASE_URL, api_level, api_level);
                        println!("Retrying with last URL format: {}", last_url);
                        let last_response = self.client.get(&last_url).send().await?;
                        if !last_response.status().is_success() {
                            return Err(format!(
                                "Failed to download SDK. Please verify the API level {} is valid.", 
                                api_level
                            ).into());
                        }
                        last_response
                    } else {
                        alt_response
                    }
                } else {
                    response
                }
            },
            Ok(Err(e)) => return Err(format!("Request error: {}", e).into()),
            Err(_) => return Err("Download timed out".into()),
        };

        let total_size = response.content_length().unwrap_or(0);
        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?);

        println!("Starting download of {} bytes", total_size);
        
        (progress_callback)(0.0);
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

        // Final progress update for download
        pb.set_position(downloaded);
        (progress_callback)(1.0);
        pb.finish_with_message("Download completed");
        println!("Starting extraction process...");
        
        // Begin extraction with its own progress tracking
        (progress_callback)(0.0);
        let temp_file = temp_file.reopen()?;
        let mut archive = zip::ZipArchive::new(temp_file)?;
        let total_files = archive.len();

        println!("Extracting {} files...", total_files);
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
            
            // Update extraction progress every 10%
            if i % (total_files / 10).max(1) == 0 {
                let progress = i as f32 / total_files as f32;
                (progress_callback)(progress);
                println!("Extraction progress: {}%", (progress * 100.0) as i32);
                tokio::task::yield_now().await;
            }
        }

        println!("Extraction completed successfully");
        (progress_callback)(1.0);
        Ok(())
    }

    pub fn get_sdk_path(&self) -> PathBuf {
        self.sdk_path.clone()
    }

    pub fn get_platform_dir(&self, api_level: &str) -> PathBuf {
        self.sdk_path.join("platforms").join(format!("android-{}", api_level))
    }
}
