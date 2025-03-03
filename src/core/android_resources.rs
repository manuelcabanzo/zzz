use std::path::PathBuf;
use directories::ProjectDirs;
use std::fs;
use serde::{Deserialize, Serialize};
use reqwest::blocking::get;
use std::io::Write;

#[derive(Debug, Serialize, Deserialize)]
pub struct AndroidResources {
    gradle_version: String,
    build_tools_version: String,
    platform_tools_version: String,
    downloaded_apis: Vec<String>,
    resources_path: PathBuf,
}

impl AndroidResources {
    pub fn new() -> Self {
        let project_dirs = ProjectDirs::from("com", "zzz", "ide")
            .expect("Failed to get project directories");
        let resources_path = project_dirs.config_dir().join("android_resources");
        
        println!("Android resources path: {}", resources_path.display());
        fs::create_dir_all(&resources_path).expect("Failed to create resources directory");
        
        Self {
            gradle_version: String::from("8.2.1"),  // Latest stable Gradle version
            build_tools_version: String::from("34.0.0"),
            platform_tools_version: String::from("34.0.5"),
            downloaded_apis: Vec::new(),
            resources_path,
        }
    }

    pub fn ensure_gradle_files(&self) -> Result<(), Box<dyn std::error::Error>> {
        let gradle_dir = self.resources_path.join("gradle");
        let wrapper_dir = gradle_dir.join("wrapper");
        fs::create_dir_all(&wrapper_dir)?;

        // First, download all files to a temporary directory
        let temp_dir = tempfile::tempdir()?;
        
        let files = vec![
            ("gradlew", "gradlew"),
            ("gradlew.bat", "gradlew.bat"),
            ("wrapper/gradle-wrapper.jar", "gradle/wrapper/gradle-wrapper.jar"),
            ("wrapper/gradle-wrapper.properties", "gradle/wrapper/gradle-wrapper.properties")
        ];

        // Download all files first
        for (dest_path, source_path) in &files {
            let url = format!(
                "https://raw.githubusercontent.com/gradle/gradle/v{}/{}",
                self.gradle_version,
                source_path
            );
            let temp_path = temp_dir.path().join(dest_path);
            
            // Create parent directories if needed
            if let Some(parent) = temp_path.parent() {
                fs::create_dir_all(parent)?;
            }

            println!("Downloading {} from {}", dest_path, url);
            let response = get(&url)?;
            if !response.status().is_success() {
                return Err(format!("Failed to download {}: {}", url, response.status()).into());
            }
            fs::write(&temp_path, response.bytes()?)?;

            // Make gradlew executable on Unix-like systems
            #[cfg(unix)]
            if dest_path == "gradlew" {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&temp_path)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&temp_path, perms)?;
            }
        }

        // If all downloads succeeded, copy files to final location
        for (dest_path, _) in files {
            let source = temp_dir.path().join(dest_path);
            let dest = if dest_path.starts_with("wrapper/") {
                gradle_dir.join(dest_path)
            } else {
                gradle_dir.join(dest_path)
            };

            // Create parent directories if needed
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }

            fs::copy(&source, &dest)?;
        }

        Ok(())
    }

    pub fn ensure_api_level(&self, api_level: &str) -> Result<(), Box<dyn std::error::Error>> {
        let api_dir = self.resources_path.join("platforms").join(format!("android-{}", api_level));
        
        if !api_dir.exists() {
            fs::create_dir_all(&api_dir)?;
            
            // Download API level files
            let url = format!(
                "https://dl.google.com/android/repository/platform-{}.zip",
                api_level
            );
            
            println!("Downloading Android API level {} from {}", api_level, url);
            let response = get(&url)?;
            let mut temp_file = tempfile::NamedTempFile::new()?;
            temp_file.write_all(&response.bytes()?)?;
            
            // Extract ZIP file
            let file = fs::File::open(temp_file.path())?;
            let mut archive = zip::ZipArchive::new(file)?;
            archive.extract(&api_dir)?;
        }

        Ok(())
    }

    pub fn get_gradle_path(&self) -> PathBuf {
        self.resources_path.join("gradle")
    }

    pub fn get_platform_path(&self, api_level: &str) -> PathBuf {
        self.resources_path.join("platforms").join(format!("android-{}", api_level))
    }

    pub fn save_state(&self) -> Result<(), Box<dyn std::error::Error>> {
        let state_file = self.resources_path.join("state.json");
        let json = serde_json::to_string_pretty(self)?;
        fs::write(state_file, json)?;
        Ok(())
    }

    pub fn load_state() -> Result<Self, Box<dyn std::error::Error>> {
        let project_dirs = ProjectDirs::from("com", "zzz", "ide")
            .expect("Failed to get project directories");
        let state_file = project_dirs.config_dir()
            .join("android_resources")
            .join("state.json");

        if state_file.exists() {
            let content = fs::read_to_string(state_file)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(Self::new())
        }
    }
}