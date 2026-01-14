//! Caching layer for AI explanations
//!
//! Stores explanations in `.covenant/explanations/` directory

use std::fs;
use std::path::PathBuf;

use sha2::{Digest, Sha256};

use crate::types::Explanation;

const GENERATOR_VERSION: &str = "0.1.0";

/// Cache for AI-generated explanations
pub struct ExplanationCache {
    cache_dir: PathBuf,
}

impl ExplanationCache {
    /// Create a new cache at the default location (.covenant/explanations/)
    pub fn new() -> Self {
        Self {
            cache_dir: PathBuf::from(".covenant/explanations"),
        }
    }

    /// Create a cache at a custom location
    pub fn with_dir(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    /// Get a cached explanation if it exists and is valid
    pub fn get(&self, snippet_id: &str, content_hash: &str) -> Option<Explanation> {
        let cache_key = self.cache_key(snippet_id, content_hash);
        let cache_path = self.cache_path(&cache_key);

        if !cache_path.exists() {
            return None;
        }

        // Read and parse
        let content = fs::read_to_string(&cache_path).ok()?;
        let explanation: Explanation = serde_json::from_str(&content).ok()?;

        // Verify hash matches
        if explanation.snippet_hash.as_deref() != Some(content_hash) {
            return None;
        }

        // Verify generator version (invalidate on version change)
        if explanation.generator_version.as_deref() != Some(GENERATOR_VERSION) {
            return None;
        }

        Some(explanation)
    }

    /// Store an explanation in the cache
    pub fn put(&self, snippet_id: &str, content_hash: &str, explanation: &Explanation) {
        let cache_key = self.cache_key(snippet_id, content_hash);
        let cache_path = self.cache_path(&cache_key);

        // Ensure directory exists
        if let Some(parent) = cache_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        // Write JSON
        if let Ok(json) = serde_json::to_string_pretty(explanation) {
            let _ = fs::write(&cache_path, json);
        }
    }

    /// Invalidate a cached explanation
    pub fn invalidate(&self, snippet_id: &str, content_hash: &str) {
        let cache_key = self.cache_key(snippet_id, content_hash);
        let cache_path = self.cache_path(&cache_key);
        let _ = fs::remove_file(cache_path);
    }

    /// Clear all cached explanations
    pub fn clear(&self) -> std::io::Result<()> {
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir)?;
        }
        Ok(())
    }

    /// List all cached snippet IDs
    pub fn list(&self) -> Vec<String> {
        let mut ids = Vec::new();

        if let Ok(entries) = fs::read_dir(&self.cache_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".json") {
                        // Try to read and extract snippet_id
                        if let Ok(content) = fs::read_to_string(entry.path()) {
                            if let Ok(exp) = serde_json::from_str::<Explanation>(&content) {
                                ids.push(exp.snippet_id);
                            }
                        }
                    }
                }
            }
        }

        ids
    }

    /// Compute cache key from snippet ID and content hash
    fn cache_key(&self, snippet_id: &str, content_hash: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(snippet_id.as_bytes());
        hasher.update(content_hash.as_bytes());
        hasher.update(GENERATOR_VERSION.as_bytes());
        format!("{:x}", hasher.finalize())[..16].to_string()
    }

    /// Get the file path for a cache key
    fn cache_path(&self, cache_key: &str) -> PathBuf {
        self.cache_dir.join(format!("{}.json", cache_key))
    }
}

impl Default for ExplanationCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::env;

    #[test]
    fn test_cache_roundtrip() {
        let temp_dir = env::temp_dir().join("covenant_cache_test");
        let cache = ExplanationCache::with_dir(temp_dir.clone());

        let explanation = Explanation {
            snippet_id: "test.example".to_string(),
            kind: "fn".to_string(),
            summary: "Test function".to_string(),
            detailed_description: None,
            parameters: Vec::new(),
            return_value: None,
            effects_summary: None,
            effects: Vec::new(),
            step_explanations: Vec::new(),
            algorithm_summary: None,
            data_flow_summary: None,
            requirements_covered: Vec::new(),
            tests_summary: None,
            related_snippets: Vec::new(),
            usage_example: None,
            warnings: Vec::new(),
            generated_at: Utc::now(),
            generator_version: Some(GENERATOR_VERSION.to_string()),
            snippet_hash: Some("abc123".to_string()),
            confidence: Some(1.0),
        };

        // Put and get
        cache.put("test.example", "abc123", &explanation);
        let retrieved = cache.get("test.example", "abc123");

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().summary, "Test function");

        // Different hash should miss
        let missed = cache.get("test.example", "different_hash");
        assert!(missed.is_none());

        // Cleanup
        let _ = cache.clear();
        let _ = fs::remove_dir_all(temp_dir);
    }
}
