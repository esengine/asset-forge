use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use xxhash_rust::xxh3::xxh3_64;

/// Cache entry for an asset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Hash of the input file content
    pub input_hash: u64,
    /// Hash of the processing configuration
    pub config_hash: u64,
    /// Path to the cached output file
    pub output_path: PathBuf,
    /// Original file modification time (Unix timestamp)
    pub mtime: u64,
    /// Processing timestamp
    pub processed_at: u64,
}

/// Asset build cache
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BuildCache {
    /// Cache entries keyed by input file path
    pub entries: HashMap<PathBuf, CacheEntry>,
    /// Cache version for invalidation on format changes
    pub version: u32,
}

const CACHE_VERSION: u32 = 1;
const CACHE_FILE_NAME: &str = "cache.json";

impl BuildCache {
    /// Create a new empty cache
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            version: CACHE_VERSION,
        }
    }

    /// Load cache from directory
    pub fn load(cache_dir: &Path) -> Result<Self> {
        let cache_file = cache_dir.join(CACHE_FILE_NAME);

        if !cache_file.exists() {
            return Ok(Self::new());
        }

        let content = std::fs::read_to_string(&cache_file)
            .with_context(|| format!("Failed to read cache file: {}", cache_file.display()))?;

        let cache: BuildCache = serde_json::from_str(&content)
            .with_context(|| "Failed to parse cache file")?;

        // Invalidate cache if version changed
        if cache.version != CACHE_VERSION {
            tracing::info!("Cache version mismatch, creating new cache");
            return Ok(Self::new());
        }

        Ok(cache)
    }

    /// Save cache to directory
    pub fn save(&self, cache_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(cache_dir)?;

        let cache_file = cache_dir.join(CACHE_FILE_NAME);
        let content = serde_json::to_string_pretty(self)?;

        std::fs::write(&cache_file, content)
            .with_context(|| format!("Failed to write cache file: {}", cache_file.display()))?;

        Ok(())
    }

    /// Check if an asset needs to be rebuilt
    pub fn needs_rebuild(
        &self,
        input: &Path,
        config_hash: u64,
    ) -> Result<bool> {
        let entry = match self.entries.get(input) {
            Some(e) => e,
            None => return Ok(true), // Not in cache
        };

        // Check if config changed
        if entry.config_hash != config_hash {
            return Ok(true);
        }

        // Check if output file exists
        if !entry.output_path.exists() {
            return Ok(true);
        }

        // Check if input file changed
        let current_hash = hash_file(input)?;
        if current_hash != entry.input_hash {
            return Ok(true);
        }

        // Check modification time as a quick check
        let metadata = std::fs::metadata(input)?;
        let mtime = get_mtime(&metadata);
        if mtime != entry.mtime {
            // mtime changed, verify with hash (already done above)
            return Ok(current_hash != entry.input_hash);
        }

        Ok(false)
    }

    /// Update cache entry after successful build
    pub fn update(
        &mut self,
        input: &Path,
        output: &Path,
        config_hash: u64,
    ) -> Result<()> {
        let input_hash = hash_file(input)?;
        let metadata = std::fs::metadata(input)?;
        let mtime = get_mtime(&metadata);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.entries.insert(
            input.to_path_buf(),
            CacheEntry {
                input_hash,
                config_hash,
                output_path: output.to_path_buf(),
                mtime,
                processed_at: now,
            },
        );

        Ok(())
    }

    /// Remove stale entries (inputs that no longer exist)
    pub fn cleanup(&mut self) {
        self.entries.retain(|path, _| path.exists());
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let total_entries = self.entries.len();
        let valid_entries = self.entries.values()
            .filter(|e| e.output_path.exists())
            .count();

        CacheStats {
            total_entries,
            valid_entries,
            stale_entries: total_entries - valid_entries,
        }
    }

    /// Clear all cache entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub valid_entries: usize,
    pub stale_entries: usize,
}

/// Hash a file's contents
pub fn hash_file(path: &Path) -> Result<u64> {
    let content = std::fs::read(path)
        .with_context(|| format!("Failed to read file for hashing: {}", path.display()))?;
    Ok(xxh3_64(&content))
}

/// Hash arbitrary data (for config hashing)
pub fn hash_data(data: &[u8]) -> u64 {
    xxh3_64(data)
}

/// Hash a configuration struct
pub fn hash_config<T: Serialize>(config: &T) -> Result<u64> {
    let json = serde_json::to_string(config)?;
    Ok(hash_data(json.as_bytes()))
}

fn get_mtime(metadata: &std::fs::Metadata) -> u64 {
    metadata.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path();

        let mut cache = BuildCache::new();
        cache.entries.insert(
            PathBuf::from("test.png"),
            CacheEntry {
                input_hash: 12345,
                config_hash: 67890,
                output_path: PathBuf::from("output/test.png"),
                mtime: 1000,
                processed_at: 2000,
            },
        );

        cache.save(cache_dir).unwrap();

        let loaded = BuildCache::load(cache_dir).unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert!(loaded.entries.contains_key(&PathBuf::from("test.png")));
    }
}
