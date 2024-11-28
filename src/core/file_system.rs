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
        fs::create_dir_all(directory)?;
        
        // Create the file
        fs::File::create(&path)?;
        
        // Invalidate cache for the parent directory
        self.invalidate_directory_cache(directory);
        
        Ok(path)
    }

    /// Opens a file and returns its content as a `String`.
    pub fn open_file(&self, path: &Path) -> io::Result<String> {
        // Check cache first
        {
            let cache = self.cache.lock().unwrap();
            if let Some((content, cached_time)) = cache.file_contents.get(path) {
                // Check if cache is fresh (within 5 minutes)
                if SystemTime::now().duration_since(*cached_time).unwrap_or(Duration::from_secs(0)) < Duration::from_secs(300) {
                    return Ok(content.clone());
                }
            }
        }

        // Added metadata check for large file prevention
        let metadata = fs::metadata(path)?;
        if metadata.len() > 10_000_000 { // 10 MB limit
            return Err(io::Error::new(
                ErrorKind::Other, 
                "File too large to open"
            ));
        }
        
        let content = fs::read_to_string(path)?;

        // Cache the file content
        {
            let mut cache = self.cache.lock().unwrap();
            cache.file_contents.insert(path.to_path_buf(), (content.clone(), SystemTime::now()));
        }

        Ok(content)
    }

    /// Saves the given content to a file with the specified path.
    pub fn save_file(&self, path: &Path, content: &str) -> io::Result<()> {
        // Ensure the parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        fs::write(path, content)?;
        
        // Update cache
        {
            let mut cache = self.cache.lock().unwrap();
            cache.file_contents.insert(path.to_path_buf(), (content.to_string(), SystemTime::now()));
        }
        
        // Invalidate directory cache for the parent directory
        self.invalidate_directory_cache(path.parent().unwrap_or(path));
        
        Ok(())
    }

    /// Lists the entries in the specified directory with caching.
    pub fn list_directory(&self, dir: &Path) -> io::Result<Vec<DirectoryEntry>> {
        // Check cache first
        {
            let cache = self.cache.lock().unwrap();
            if let Some(cached_entries) = self.get_cached_directory_entries(&cache, dir) {
                return Ok(cached_entries);
            }
        }

        // If not in cache, read from file system
        let mut entries = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            // Get file metadata
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
        if let Some(new_parent) = new_path.parent() {
            fs::create_dir_all(new_parent)?;
        }

        // Rename the file/directory
        fs::rename(old_path, new_path)?;

        // Invalidate caches for both old and new parent directories
        if let Some(old_parent) = old_path.parent() {
            self.invalidate_directory_cache(old_parent);
        }
        if let Some(new_parent) = new_path.parent() {
            self.invalidate_directory_cache(new_parent);
        }

        // Update file content cache if applicable
        {
            let mut cache = self.cache.lock().unwrap();
            if let Some((content, _)) = cache.file_contents.remove(old_path) {
                cache.file_contents.insert(new_path.to_path_buf(), (content, SystemTime::now()));
            }
        }

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
        if let Some(parent) = path.parent() {
            self.invalidate_directory_cache(parent);
        }

        // Remove from file contents cache
        {
            let mut cache = self.cache.lock().unwrap();
            cache.file_contents.remove(path);
        }

        Ok(())
    }

    /// Creates a new directory at the specified path.
    pub fn create_directory(&self, path: &Path) -> io::Result<()> {
        // Create directory and any necessary parent directories
        fs::create_dir_all(path)?;
        
        // Invalidate cache for the parent directory
        if let Some(parent) = path.parent() {
            self.invalidate_directory_cache(parent);
        }
        
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

    // Private method to get cached directory entries
    fn get_cached_directory_entries(&self, cache: &FileSystemCache, dir: &Path) -> Option<Vec<DirectoryEntry>> {
        // Check if cache exists and is recent (within 5 minutes)
        cache.last_updated.get(dir).and_then(|last_updated| {
            SystemTime::now().duration_since(*last_updated)
                .map(|duration| duration.as_secs() < 300) // 5 minutes cache timeout
                .unwrap_or(false)
                .then(|| cache.directory_contents.get(dir).cloned())
                .flatten()
        })
    }

    // Private method to cache directory entries
    fn cache_directory_entries(&self, dir: &Path, entries: &[DirectoryEntry]) {
        let mut cache = self.cache.lock().unwrap();
        cache.directory_contents.insert(dir.to_path_buf(), entries.to_vec());
        cache.last_updated.insert(dir.to_path_buf(), SystemTime::now());
    }

    // Private method to invalidate directory cache
    fn invalidate_directory_cache(&self, dir: &Path) {
        let mut cache = self.cache.lock().unwrap();
        cache.directory_contents.remove(dir);
        cache.last_updated.remove(dir);
    }
}