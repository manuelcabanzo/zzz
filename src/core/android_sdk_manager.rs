use std::path::PathBuf;
use std::fs::{self, File};
use std::io::Write;
use std::time::Duration;
use reqwest::Client;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::time::timeout;
use std::sync::Arc;

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes timeout

pub struct AndroidSdkManager {
    sdk_path: PathBuf,
    client: Client,
}

impl AndroidSdkManager {
    pub fn new() -> Self {
        let home_dir = dirs::home_dir().expect("Could not find home directory");
        let sdk_path = home_dir.join(".zzz").join("android-sdk");
        fs::create_dir_all(&sdk_path).expect("Could not create SDK directory");

        Self {
            sdk_path,
            client: Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .expect("Could not create HTTP client"),
        }
    }

    pub async fn ensure_api_level(&self, api_level: &str, progress_callback: Arc<dyn Fn(f32) + Send + Sync>) -> Result<(), Box<dyn std::error::Error>> {
        let platform_dir = self.sdk_path.join("platforms").join(format!("android-{}", api_level));
        
        if platform_dir.exists() {
            (progress_callback)(1.0);
            return Ok(());
        }

        fs::create_dir_all(&platform_dir)?;

        let url = format!(
            "https://dl.google.com/android/repository/platform-{}.zip",
            api_level
        );

        let total_size = self.get_download_size(&url).await?;
        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?);

        let response = timeout(DOWNLOAD_TIMEOUT, self.client.get(&url).send()).await??;
        let bytes = response.bytes().await?;
        let downloaded = bytes.len() as u64;
        
        let mut temp_file = tempfile::NamedTempFile::new()?;
        temp_file.write_all(&bytes)?;
        
        pb.set_position(downloaded);
        (progress_callback)(downloaded as f32 / total_size as f32);

        pb.finish_with_message("Download completed");

        println!("Extracting SDK files...");
        let mut archive = zip::ZipArchive::new(File::open(temp_file.path())?)?;
        archive.extract(&platform_dir)?;

        println!("API level {} installation completed", api_level);
        (progress_callback)(1.0);
        Ok(())
    }

    async fn get_download_size(&self, url: &str) -> Result<u64, Box<dyn std::error::Error>> {
        let response = self.client.head(url).send().await?;
        Ok(response.content_length().unwrap_or(0))
    }

    pub fn get_sdk_path(&self) -> PathBuf {
        self.sdk_path.clone()
    }

    pub fn get_platform_dir(&self, api_level: &str) -> PathBuf {
        self.sdk_path.join("platforms").join(format!("android-{}", api_level))
    }
}
