use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct DirectoryEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: SystemTime,
}

#[derive(Clone)]
pub struct FileSystem {
    project_directory: PathBuf,
    cache: Arc<Mutex<FileSystemCache>>,
}

#[derive(Default)]
struct FileSystemCache {
    directory_contents: HashMap<PathBuf, Vec<DirectoryEntry>>,
    last_updated: HashMap<PathBuf, SystemTime>,
    file_contents: HashMap<PathBuf, (String, SystemTime)>,
}

impl FileSystem {
    const CACHE_TIMEOUT_SECS: u64 = 300; // 5 minutes
    const MAX_FILE_SIZE_BYTES: u64 = 10_000_000; // 10 MB

    /// Creates a new `FileSystem` instance with the given project directory.
    pub fn new(project_directory: &str) -> Self {
        Self {
            project_directory: PathBuf::from(project_directory),
            cache: Arc::new(Mutex::new(FileSystemCache::default())),
        }
    }

    /// Creates a new file with the specified filename in the given directory.
    pub fn create_new_file(&self, directory: &Path, filename: &str) -> io::Result<PathBuf> {
        let path = directory.join(filename);

        // Ensure the directory exists
        self.ensure_directory_exists(directory)?;

        // Create the file
        fs::File::create(&path)?;

        // Invalidate cache for the parent directory
        self.invalidate_directory_cache(directory);

        Ok(path)
    }

    /// Opens a file and returns its content as a `String`.
    pub fn open_file(&self, path: &Path) -> io::Result<String> {
        // Check cache first
        if let Some(content) = self.get_cached_file_content(path)? {
            return Ok(content);
        }

        // Check file size before reading
        let metadata = fs::metadata(path)?;
        if metadata.len() > Self::MAX_FILE_SIZE_BYTES {
            return Err(io::Error::new(
                ErrorKind::Other,
                format!("File too large to open ({} bytes)", metadata.len()),
            ));
        }

        // Read file content
        let content = fs::read_to_string(path)?;

        // Cache the file content
        self.cache_file_content(path, &content);

        Ok(content)
    }

    /// Saves the given content to a file with the specified path.
    pub fn save_file(&self, path: &Path, content: &str) -> io::Result<()> {
        // Ensure the parent directory exists
        self.ensure_directory_exists(path.parent().unwrap_or(path))?;

        // Write content to file
        fs::write(path, content)?;

        // Update cache
        self.cache_file_content(path, content);

        // Invalidate directory cache for the parent directory
        self.invalidate_directory_cache(path.parent().unwrap_or(path));

        Ok(())
    }

    /// Lists the entries in the specified directory with caching.
    pub fn list_directory(&self, dir: &Path) -> io::Result<Vec<DirectoryEntry>> {
        // Check cache first
        if let Some(entries) = self.get_cached_directory_entries(dir)? {
            return Ok(entries);
        }

        // If not in cache, read from file system
        let mut entries = self.read_directory_entries(dir)?;

        // Sort entries (directories first, then alphabetically)
        entries.sort_by(|a, b| {
            if a.is_dir == b.is_dir {
                a.name.cmp(&b.name)
            } else {
                b.is_dir.cmp(&a.is_dir)
            }
        });

        // Cache the results
        self.cache_directory_entries(dir, &entries);

        Ok(entries)
    }

    /// Renames a file or directory from `old_path` to `new_path`.
    pub fn rename_file(&self, old_path: &Path, new_path: &Path) -> io::Result<()> {
        // Ensure parent directories exist
        self.ensure_directory_exists(new_path.parent().unwrap_or(new_path))?;

        // Rename the file/directory
        fs::rename(old_path, new_path)?;

        // Invalidate caches for both old and new parent directories
        self.invalidate_directory_cache(old_path.parent().unwrap_or(old_path));
        self.invalidate_directory_cache(new_path.parent().unwrap_or(new_path));

        // Update file content cache if applicable
        self.update_file_content_cache(old_path, new_path);

        Ok(())
    }

    /// Deletes a file or directory at the specified path.
    pub fn delete_file(&self, path: &Path) -> io::Result<()> {
        // Determine if it's a directory or file
        let is_dir = path.is_dir();

        // Delete the file or directory
        if is_dir {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }

        // Invalidate cache for the parent directory
        self.invalidate_directory_cache(path.parent().unwrap_or(path));

        // Remove from file contents cache
        self.remove_file_content_cache(path);

        Ok(())
    }

    /// Creates a new directory at the specified path.
    pub fn create_directory(&self, path: &Path) -> io::Result<()> {
        // Create directory and any necessary parent directories
        fs::create_dir_all(path)?;

        // Invalidate cache for the parent directory
        self.invalidate_directory_cache(path.parent().unwrap_or(path));

        Ok(())
    }

    /// Returns the project directory.
    pub fn get_project_directory(&self) -> &Path {
        &self.project_directory
    }

    /// Checks if a path exists.
    pub fn path_exists(&self, path: &Path) -> bool {
        path.exists()
    }

    // Helper methods

    fn ensure_directory_exists(&self, directory: &Path) -> io::Result<()> {
        if !directory.exists() {
            fs::create_dir_all(directory)?;
        }
        Ok(())
    }

    fn get_cached_file_content(&self, path: &Path) -> io::Result<Option<String>> {
        let cache = self.cache.lock().unwrap();
        if let Some((content, cached_time)) = cache.file_contents.get(path) {
            if SystemTime::now().duration_since(*cached_time).unwrap_or(Duration::from_secs(0))
                < Duration::from_secs(Self::CACHE_TIMEOUT_SECS)
            {
                return Ok(Some(content.clone()));
            }
        }
        Ok(None)
    }

    fn cache_file_content(&self, path: &Path, content: &str) {
        let mut cache = self.cache.lock().unwrap();
        cache.file_contents.insert(path.to_path_buf(), (content.to_string(), SystemTime::now()));
    }

    fn get_cached_directory_entries(&self, dir: &Path) -> io::Result<Option<Vec<DirectoryEntry>>> {
        let cache = self.cache.lock().unwrap();
        if let Some(last_updated) = cache.last_updated.get(dir) {
            if SystemTime::now().duration_since(*last_updated).unwrap_or(Duration::from_secs(0))
                < Duration::from_secs(Self::CACHE_TIMEOUT_SECS)
            {
                return Ok(cache.directory_contents.get(dir).cloned());
            }
        }
        Ok(None)
    }

    fn cache_directory_entries(&self, dir: &Path, entries: &[DirectoryEntry]) {
        let mut cache = self.cache.lock().unwrap();
        cache.directory_contents.insert(dir.to_path_buf(), entries.to_vec());
        cache.last_updated.insert(dir.to_path_buf(), SystemTime::now());
    }

    fn invalidate_directory_cache(&self, dir: &Path) {
        let mut cache = self.cache.lock().unwrap();
        cache.directory_contents.remove(dir);
        cache.last_updated.remove(dir);
    }

    fn update_file_content_cache(&self, old_path: &Path, new_path: &Path) {
        let mut cache = self.cache.lock().unwrap();
        if let Some((content, _)) = cache.file_contents.remove(old_path) {
            cache.file_contents.insert(new_path.to_path_buf(), (content, SystemTime::now()));
        }
    }

    fn remove_file_content_cache(&self, path: &Path) {
        let mut cache = self.cache.lock().unwrap();
        cache.file_contents.remove(path);
    }

    fn read_directory_entries(&self, dir: &Path) -> io::Result<Vec<DirectoryEntry>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = entry.metadata()?;

            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                entries.push(DirectoryEntry {
                    name: name.to_string(),
                    is_dir: metadata.is_dir(),
                    size: metadata.len(),
                    modified: metadata.modified().unwrap_or(UNIX_EPOCH),
                });
            }
        }
        Ok(entries)
    }
}