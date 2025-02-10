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
        
        // Create resources directory if it doesn't exist
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
        fs::create_dir_all(&gradle_dir)?;

        // URLs for Gradle files
        let files = vec![
            ("gradlew", "https://raw.githubusercontent.com/gradle/gradle/v{}/gradlew"),
            ("gradlew.bat", "https://raw.githubusercontent.com/gradle/gradle/v{}/gradlew.bat"),
            ("gradle-wrapper.jar", "https://raw.githubusercontent.com/gradle/gradle/v{}/gradle/wrapper/gradle-wrapper.jar"),
            ("gradle-wrapper.properties", "https://raw.githubusercontent.com/gradle/gradle/v{}/gradle/wrapper/gradle-wrapper.properties"),
        ];

        for (filename, url_template) in files {
            let file_path = gradle_dir.join(filename);
            if !file_path.exists() {
                let url = url_template.replace("{}", &self.gradle_version);
                println!("Downloading {} from {}", filename, url);
                let response = get(&url)?;
                let mut file = fs::File::create(&file_path)?;
                file.write_all(&response.bytes()?)?;
            }
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