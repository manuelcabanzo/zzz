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

#[derive(Clone)]
pub struct GitManager {
    repo_path: PathBuf,
    is_checking_out: Arc<AtomicBool>,
}

impl GitManager {
    const GIT_LOG_FORMAT: &'static str = "%H|||%an|||%ai|||%s";
    const DATE_FORMAT_ISO: &'static str = "%Y-%m-%d %H:%M:%S %z";

    pub fn new(repo_path: PathBuf) -> Self {
        Self {
            repo_path,
            is_checking_out: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Checks if the current directory is a valid Git repository.
    pub fn is_git_repo(&self) -> bool {
        let git_dir = self.repo_path.join(".git");
        let direct_check = git_dir.exists() && git_dir.is_dir();

        println!("Checking git repo at: {}", self.repo_path.display());
        println!("Direct check: {}", direct_check);

        if direct_check {
            return true;
        }

        match Self::run_git_command(&["rev-parse", "--git-dir"], &self.repo_path) {
            Ok(output) => {
                println!("Git rev-parse output: {:?}", output);
                output.status.success()
            }
            Err(e) => {
                println!("Git command execution error: {}", e);
                false
            }
        }
    }

    /// Initializes the Git repository if it exists.
    pub fn initialize(&self) -> Result<(), String> {
        println!("Initializing git repo at: {}", self.repo_path.display());

        match Self::run_git_command(&["status"], &self.repo_path) {
            Ok(output) => {
                println!("Git status output: {:?}", output);
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    println!("Git status failed: {}", stderr);
                    return Err(format!("Git status failed: {}", stderr));
                }
                Ok(())
            }
            Err(e) => {
                println!("Git status command error: {}", e);
                Err(format!("Failed to execute git command: {}", e))
            }
        }
    }

    /// Retrieves the list of commits from the Git repository.
    pub fn get_commits(&self) -> Result<Vec<GitCommit>, String> {
        if !self.is_git_repo() {
            return Err("Not a git repository".to_string());
        }

        let output = Self::run_git_command(
            &[
                "log",
                &format!("--pretty=format:{}", Self::GIT_LOG_FORMAT),
                "--date=iso",
                "--all",
            ],
            &self.repo_path,
        )?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to get git history: {}", stderr));
        }

        let output_str = String::from_utf8(output.stdout).map_err(|e| format!("Invalid UTF-8 in git output: {}", e))?;
        if output_str.is_empty() {
            return Ok(Vec::new());
        }

        let commits = output_str
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| Self::parse_commit_line(line))
            .collect::<Result<Vec<_>, String>>()?;

        Ok(commits)
    }

    pub fn reset_to_commit(&self, commit_hash: &str) -> Result<(), String> {
        if self.is_checking_out.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            return Err("Another operation is in progress".to_string());
        }

        let result = self.perform_reset(commit_hash);
        self.is_checking_out.store(false, Ordering::SeqCst);
        result
    }

    fn perform_reset(&self, commit_hash: &str) -> Result<(), String> {
        self.run_git_command_with_check(
            &["reset", "--hard", commit_hash],
            &format!("Failed to reset to commit {}", commit_hash)
        )
    }

    fn run_git_command(args: &[&str], repo_path: &PathBuf) -> Result<std::process::Output, String> {
        Command::new("git")
            .args(args)
            .current_dir(repo_path)
            .output()
            .map_err(|e| format!("Failed to execute git command: {}", e))
    }

    fn run_git_command_with_check(&self, args: &[&str], error_message: &str) -> Result<(), String> {
        let output = Self::run_git_command(args, &self.repo_path)?;
        if !output.status.success() {
            return Err(format!("{}: {}", error_message, String::from_utf8_lossy(&output.stderr)));
        }
        Ok(())
    }

    fn parse_commit_line(line: &str) -> Result<GitCommit, String> {
        let parts: Vec<&str> = line.split("|||").collect();
        if parts.len() != 4 {
            return Err(format!("Invalid commit line format: '{}'", line));
        }

        let date = DateTime::parse_from_rfc3339(parts[2])
            .or_else(|_| DateTime::parse_from_str(parts[2], Self::DATE_FORMAT_ISO))
            .map_err(|e| format!("Failed to parse date '{}': {}", parts[2], e))?
            .with_timezone(&Local);

        Ok(GitCommit {
            hash: parts[0].to_string(),
            author: parts[1].to_string(),
            date,
            message: parts[3].to_string(),
        })
    }
}