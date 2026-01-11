//! File watching and incremental synchronization

use crate::{Result, StorageProvider, StorageError};
use notify::{Watcher, RecursiveMode, Event, EventKind};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

/// Storage synchronization manager
///
/// Watches for file system changes and incrementally updates the storage index.
pub struct StorageSync<P: StorageProvider> {
    storage: P,
    project_root: PathBuf,
    watcher: Option<Box<dyn Watcher + Send>>,
}

impl<P: StorageProvider> StorageSync<P> {
    /// Create a new sync manager with the given storage and project root
    pub fn new(storage: P, project_root: PathBuf) -> Self {
        Self {
            storage,
            project_root,
            watcher: None,
        }
    }

    /// Get a reference to the underlying storage
    pub fn storage(&self) -> &P {
        &self.storage
    }

    /// Get a mutable reference to the underlying storage
    pub fn storage_mut(&mut self) -> &mut P {
        &mut self.storage
    }

    /// Start watching for file changes
    ///
    /// This spawns a background thread that monitors `.cov` files
    /// and updates the storage index incrementally.
    pub fn start_watching(&mut self) -> Result<Receiver<notify::Result<Event>>> {
        let (tx, rx) = channel();

        let mut watcher = notify::recommended_watcher(tx)
            .map_err(|e| StorageError::FileWatcher(e.to_string()))?;

        watcher
            .watch(&self.project_root, RecursiveMode::Recursive)
            .map_err(|e| StorageError::FileWatcher(e.to_string()))?;

        self.watcher = Some(Box::new(watcher));

        Ok(rx)
    }

    /// Process a file change event
    pub fn process_event(&mut self, event: Event) -> Result<()> {
        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                for path in event.paths {
                    if path.extension().and_then(|s| s.to_str()) == Some("cov") {
                        self.sync_file(&path)?;
                    }
                }
            }
            EventKind::Remove(_) => {
                for path in event.paths {
                    if path.extension().and_then(|s| s.to_str()) == Some("cov") {
                        self.remove_file(&path)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Synchronize a single `.cov` file
    ///
    /// Parses the file and updates all snippets it contains.
    pub fn sync_file(&mut self, path: &Path) -> Result<()> {
        // TODO: This needs the parser to be implemented
        // For now, this is a placeholder that shows the structure

        let _source = std::fs::read_to_string(path)?;

        // Parse snippets from the source
        // let snippets = parse_snippets(&source)?;

        // For each snippet, create a Node and store it
        // for snippet in snippets {
        //     let node = Node::from_snippet(snippet, path)?;
        //     self.storage.put(&node.id, &node)?;
        // }

        // Placeholder: Just log that we would sync this file
        eprintln!("Would sync file: {}", path.display());

        Ok(())
    }

    /// Remove all snippets from a deleted file
    pub fn remove_file(&mut self, path: &Path) -> Result<()> {
        // Find all nodes from this file
        let all_nodes = self.storage.list("")?;

        // Extract just the filename for comparison
        let filename = path.file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| StorageError::InvalidPath(path.to_string_lossy().to_string()))?;

        for node_id in all_nodes {
            if let Some(node) = self.storage.get(&node_id)? {
                // Compare against the filename only
                let node_filename = Path::new(&node.source_file)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");

                if node_filename == filename {
                    self.storage.delete(&node_id)?;
                }
            }
        }

        Ok(())
    }

    /// Perform a full rebuild of the index from all `.cov` files
    pub fn rebuild_index(&mut self) -> Result<()> {
        use walkdir::WalkDir;

        // Scan all .cov files
        for entry in WalkDir::new(&self.project_root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("cov"))
        {
            self.sync_file(entry.path())?;
        }

        Ok(())
    }

    /// Stop watching for file changes
    pub fn stop_watching(&mut self) {
        self.watcher = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InMemoryStorage, SnippetKind, Node};
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_remove_file() {
        let dir = tempdir().unwrap();
        let project_root = dir.path().to_path_buf();

        let mut storage = InMemoryStorage::new();

        // Add some nodes
        let mut node1 = Node::new("test.func1", SnippetKind::Function);
        node1.source_file = "test.cov".to_string();

        let mut node2 = Node::new("test.func2", SnippetKind::Function);
        node2.source_file = "test.cov".to_string();

        let mut node3 = Node::new("other.func", SnippetKind::Function);
        node3.source_file = "other.cov".to_string();

        storage.put("test.func1", &node1).unwrap();
        storage.put("test.func2", &node2).unwrap();
        storage.put("other.func", &node3).unwrap();

        let mut sync = StorageSync::new(storage, project_root.clone());

        // Remove test.cov
        let test_cov_path = project_root.join("test.cov");
        sync.remove_file(&test_cov_path).unwrap();

        // Check that test.func1 and test.func2 are gone
        assert!(sync.storage().get("test.func1").unwrap().is_none());
        assert!(sync.storage().get("test.func2").unwrap().is_none());

        // Check that other.func is still there
        assert!(sync.storage().get("other.func").unwrap().is_some());
    }

    #[test]
    fn test_rebuild_index() {
        let dir = tempdir().unwrap();
        let project_root = dir.path().to_path_buf();

        // Create a test .cov file
        let test_file = project_root.join("test.cov");
        fs::write(&test_file, "# Test file\n").unwrap();

        let storage = InMemoryStorage::new();
        let mut sync = StorageSync::new(storage, project_root.clone());

        // Rebuild should not fail (even though parsing is not implemented yet)
        let result = sync.rebuild_index();
        assert!(result.is_ok());
    }
}
