use std::process::Command;
use std::path::PathBuf;
use chrono::{DateTime, Local};
use serde::{Serialize, Deserialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCommit {
    pub hash: String,
    pub author: String,
    pub date: DateTime<Local>,
    pub message: String,
}

pub struct GitManager {
    repo_path: PathBuf,
    is_checking_out: Arc<AtomicBool>,
}

impl GitManager {
    pub fn new(repo_path: PathBuf) -> Self {
        Self { 
            repo_path,
            is_checking_out: Arc::new(AtomicBool::new(false))
        }
    }

    pub fn is_git_repo(&self) -> bool {
        // Enhanced logging version
        let git_dir = self.repo_path.join(".git");
        let direct_check = git_dir.exists() && git_dir.is_dir();
        
        println!("Checking git repo at: {}", self.repo_path.display()); // Add this
        println!("Direct check: {}", direct_check); // Add this

        if direct_check {
            return true;
        }

        // Enhanced command execution logging
        let output = Command::new("git")
            .args(&["rev-parse", "--git-dir"])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| {
                println!("Git command execution error: {}", e); // Add this
                e
            });

        match output {
            Ok(output) => {
                println!("Git rev-parse output: {:?}", output); // Add this
                output.status.success()
            },
            Err(_) => false
        }
    }

    pub fn initialize(&self) -> Result<(), String> {
        println!("Initializing git repo at: {}", self.repo_path.display()); // Add this
        
        let output = Command::new("git")
            .args(&["status"])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| {
                println!("Git status command error: {}", e); // Add this
                format!("Failed to execute git command: {}", e)
            })?;

        println!("Git status output: {:?}", output); // Add this
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("Git status failed: {}", stderr); // Add this
            return Err(format!("Git status failed: {}", stderr));
        }

        Ok(())
    }

    pub fn get_commits(&self) -> Result<Vec<GitCommit>, String> {
        if !self.is_git_repo() {
            return Err("Not a git repository".to_string());
        }

        let output = Command::new("git")
            .args(&[
                "log",
                "--pretty=format:%H|||%an|||%ai|||%s",
                "--date=iso",
                "--all"  // Add this to show commits from all branches
            ])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| format!("Git command failed: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to get git history: {}", stderr));
        }

        let output_str = String::from_utf8(output.stdout)
            .map_err(|e| format!("Invalid UTF-8 in git output: {}", e))?;

        if output_str.is_empty() {
            return Ok(Vec::new());
        }

        let commits = output_str
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| {
                let parts: Vec<&str> = line.split("|||").collect();
                if parts.len() != 4 {
                    return Err(format!("Invalid commit line format: '{}'", line));
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
    
    pub fn checkout_commit(&self, commit_hash: &str) -> Result<(), String> {
        // Prevent multiple concurrent checkouts
        if self.is_checking_out.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            return Err("Another checkout operation is in progress".to_string());
        }

        let result = self.perform_checkout(commit_hash);
        self.is_checking_out.store(false, Ordering::SeqCst);
        result
    }

    fn perform_checkout(&self, commit_hash: &str) -> Result<(), String> {
        // Reset any local changes
        let reset_output = Command::new("git")
            .args(&["reset", "--hard", "HEAD"])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| format!("Failed to reset: {}", e))?;
    
        if !reset_output.status.success() {
            return Err("Failed to reset changes".to_string());
        }
    
        // Clean untracked files
        let clean_output = Command::new("git")
            .args(&["clean", "-fd"])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| format!("Failed to clean: {}", e))?;
    
        if !clean_output.status.success() {
            return Err("Failed to clean repository".to_string());
        }
    
        // Checkout with force
        let checkout_output = Command::new("git")
            .args(&["checkout", "--force", commit_hash])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| format!("Failed to checkout: {}", e))?;
    
        if !checkout_output.status.success() {
            return Err(format!("Checkout failed: {}", 
                String::from_utf8_lossy(&checkout_output.stderr)));
        }
    
        Ok(())
    }

    pub fn is_checkout_in_progress(&self) -> bool {
        self.is_checking_out.load(Ordering::SeqCst)
    }
}
