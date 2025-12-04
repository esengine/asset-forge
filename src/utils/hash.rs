use anyhow::Result;
use std::path::Path;
use xxhash_rust::xxh3::xxh3_64;

/// Compute a hash of a file's contents for incremental build tracking
pub fn hash_file(path: &Path) -> Result<u64> {
    let content = std::fs::read(path)?;
    Ok(xxh3_64(&content))
}

/// Compute a hash from multiple inputs (for cache key generation)
pub fn hash_inputs(inputs: &[&[u8]]) -> u64 {
    let mut combined = Vec::new();
    for input in inputs {
        combined.extend_from_slice(input);
    }
    xxh3_64(&combined)
}
