use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub struct FileSystem {
    project_directory: PathBuf,
}

pub struct DirectoryEntry {
    pub name: String,
    pub is_dir: bool,
}

impl FileSystem {
    /// Creates a new `FileSystem` instance with the given project directory.
    pub fn new(project_directory: &str) -> Self {
        Self {
            project_directory: PathBuf::from(project_directory),
        }
    }

    /// Creates a new file with the specified filename in the given directory.
    pub fn create_new_file(&self, directory: &Path, filename: &str) -> io::Result<()> {
        let path = directory.join(filename);
        fs::write(&path, "")?;
        Ok(())
    }

    /// Opens a file and returns its content as a `String`.
    pub fn open_file(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }

    /// Saves the given content to a file with the specified path.
    pub fn save_file(&self, path: &Path, content: &str) -> io::Result<()> {
        fs::write(path, content)
    }

    /// Lists the entries in the specified directory.
    pub fn list_directory(&self, dir: &Path) -> io::Result<Vec<DirectoryEntry>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                entries.push(DirectoryEntry {
                    name: name.to_string(),
                    is_dir: path.is_dir(),
                });
            }
        }
        entries.sort_by(|a, b| {
            if a.is_dir == b.is_dir {
                a.name.cmp(&b.name)
            } else {
                b.is_dir.cmp(&a.is_dir)
            }
        });
        Ok(entries)
    }

    /// Renames a file or directory from `old_path` to `new_path`.
    pub fn rename_file(&self, old_path: &Path, new_path: &Path) -> io::Result<()> {
        fs::rename(old_path, new_path)
    }

    /// Deletes a file or directory at the specified path.
    pub fn delete_file(&self, path: &Path) -> io::Result<()> {
        if path.is_dir() {
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        }
    }

    /// Creates a new directory at the specified path.
    #[allow(dead_code)]
    pub fn create_directory(&self, path: &Path) -> io::Result<()> {
        fs::create_dir(path)
    }

    /// Returns the project directory.
    pub fn get_project_directory(&self) -> &Path {
        &self.project_directory
    }
    
    /// Checks if a path exists.
    #[allow(dead_code)]
    pub fn path_exists(&self, path: &Path) -> bool {
        path.exists()
    }
}
