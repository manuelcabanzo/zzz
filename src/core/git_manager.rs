use std::process::Command;
use std::path::PathBuf;
use chrono::{DateTime, Local};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCommit {
    pub hash: String,
    pub author: String,
    pub date: DateTime<Local>,
    pub message: String,
}

pub struct GitManager {
    repo_path: PathBuf,
}

impl GitManager {
    pub fn new(repo_path: PathBuf) -> Self {
        Self { repo_path }
    }

    pub fn is_git_repo(&self) -> bool {
        // First check the direct path
        let git_dir = self.repo_path.join(".git");
        let direct_check = git_dir.exists() && git_dir.is_dir();
        
        if direct_check {
            return true;
        }

        // If direct check fails, try git rev-parse
        let output = Command::new("git")
            .args(&["rev-parse", "--git-dir"])
            .current_dir(&self.repo_path)
            .output();

        match output {
            Ok(output) => output.status.success(),
            Err(_) => false
        }
    }

    pub fn get_commits(&self) -> Result<Vec<GitCommit>, String> {
        // First verify it's a git repo
        if !self.is_git_repo() {
            return Err("Not a git repository".to_string());
        }

        let output = Command::new("git")
            .args(&[
                "log",
                "--pretty=format:%H|||%an|||%ai|||%s",  // Changed date format to ISO
                "--date=iso"
            ])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| format!("Git command failed: {}", e))?;

        if !output.status.success() {
            return Err("Failed to get git history".to_string());
        }

        let output_str = String::from_utf8(output.stdout)
            .map_err(|e| format!("Invalid UTF-8 in git output: {}", e))?;

        let commits = output_str
            .lines()
            .map(|line| {
                let parts: Vec<&str> = line.split("|||").collect();
                if parts.len() != 4 {
                    return Err(format!("Invalid commit line format: {}", line));
                }

                let date = DateTime::parse_from_rfc3339(parts[2])
                    .or_else(|_| DateTime::parse_from_str(parts[2], "%Y-%m-%d %H:%M:%S %z"))
                    .map_err(|e| format!("Failed to parse date '{}': {}", parts[2], e))?
                    .with_timezone(&Local);

                Ok(GitCommit {
                    hash: parts[0].to_string(),
                    author: parts[1].to_string(),
                    date,
                    message: parts[3].to_string(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;

        Ok(commits)
    }
    pub fn initialize(&self) -> Result<(), String> {
        if !self.is_git_repo() {
            return Err("Not a git repository".to_string());
        }
        
        // Test git command execution
        let output = Command::new("git")
            .args(&["status"])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| format!("Failed to execute git command: {}", e))?;

        if !output.status.success() {
            return Err("Git status command failed".to_string());
        }

        Ok(())
    }
    
    pub fn checkout_commit(&self, commit_hash: &str) -> Result<(), String> {
        // First stash any current changes
        let _ = Command::new("git")
            .args(&["stash"])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| e.to_string())?;

        // Checkout the specific commit
        let output = Command::new("git")
            .args(&["checkout", commit_hash])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| e.to_string())?;

        if !output.status.success() {
            return Err("Failed to checkout commit".to_string());
        }

        Ok(())
    }
}
