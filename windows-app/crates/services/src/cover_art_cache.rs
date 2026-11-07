//! Disk + memory art cache (spec task 12.3, R18.3), shared by [`crate::cover_art_cache`] (keyed
//! by track path) and [`crate::artist_art_cache`] (keyed by `"artist|album"`).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::error::Result;

/// A generic two-tier (in-memory + on-disk) byte cache, keyed by an arbitrary string. Disk
/// filenames are a hash of the key rather than a sanitized version of it, so arbitrarily-shaped
/// keys (file paths, `"artist|album"` pairs) never collide or need their own escaping scheme.
pub struct ArtCache {
    memory: Mutex<HashMap<String, Arc<Vec<u8>>>>,
    disk_dir: PathBuf,
}

fn hash_key(key: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

impl ArtCache {
    pub fn new(disk_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&disk_dir)?;
        Ok(Self { memory: Mutex::new(HashMap::new()), disk_dir })
    }

    fn disk_path(&self, key: &str) -> PathBuf {
        self.disk_dir.join(format!("{}.jpg", hash_key(key)))
    }

    /// Checks memory first, then disk (populating memory on a disk hit).
    pub fn get(&self, key: &str) -> Option<Arc<Vec<u8>>> {
        if let Some(bytes) = self.memory.lock().unwrap().get(key) {
            return Some(Arc::clone(bytes));
        }
        let bytes = std::fs::read(self.disk_path(key)).ok()?;
        let arc = Arc::new(bytes);
        self.memory.lock().unwrap().insert(key.to_string(), Arc::clone(&arc));
        Some(arc)
    }

    /// Writes `bytes` to both tiers.
    pub fn put(&self, key: &str, bytes: Vec<u8>) -> Result<()> {
        std::fs::write(self.disk_path(key), &bytes)?;
        self.memory.lock().unwrap().insert(key.to_string(), Arc::new(bytes));
        Ok(())
    }

    pub fn contains(&self, key: &str) -> bool {
        self.memory.lock().unwrap().contains_key(key) || self.disk_path(key).exists()
    }

    /// Drops the in-memory tier only (e.g. to free RAM under memory pressure); disk entries are
    /// untouched and will repopulate memory on next access.
    pub fn clear_memory(&self) {
        self.memory.lock().unwrap().clear();
    }
}

/// Cover-art cache, keyed by the track's full file path.
pub type CoverArtCache = ArtCache;

pub fn cover_art_cache_key(track_full_path: &str) -> String {
    track_full_path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_then_get_round_trips_through_memory() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ArtCache::new(dir.path().to_path_buf()).unwrap();
        cache.put("key-1", vec![1, 2, 3]).unwrap();
        assert_eq!(cache.get("key-1").map(|b| (*b).clone()), Some(vec![1, 2, 3]));
    }

    #[test]
    fn get_falls_back_to_disk_after_memory_is_cleared() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ArtCache::new(dir.path().to_path_buf()).unwrap();
        cache.put("key-1", vec![9, 9, 9]).unwrap();
        cache.clear_memory();

        assert_eq!(cache.get("key-1").map(|b| (*b).clone()), Some(vec![9, 9, 9]));
    }

    #[test]
    fn missing_key_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ArtCache::new(dir.path().to_path_buf()).unwrap();
        assert!(cache.get("nope").is_none());
        assert!(!cache.contains("nope"));
    }

    #[test]
    fn keys_with_path_separators_and_pipes_dont_collide_or_escape_the_cache_dir() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ArtCache::new(dir.path().to_path_buf()).unwrap();
        cache.put(r"C:\Music\Artist\Album\song.flac", vec![1]).unwrap();
        cache.put("Artist|Album", vec![2]).unwrap();

        assert_eq!(cache.get(r"C:\Music\Artist\Album\song.flac").map(|b| (*b).clone()), Some(vec![1]));
        assert_eq!(cache.get("Artist|Album").map(|b| (*b).clone()), Some(vec![2]));
        for entry in std::fs::read_dir(dir.path()).unwrap() {
            let name = entry.unwrap().file_name().to_string_lossy().into_owned();
            assert!(!name.contains(['\\', '/', '|']), "disk filename leaked raw key chars: {name}");
        }
    }
}
