use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Disk-based response cache with configurable TTL.
///
/// Caches HTTP response text as JSON files keyed by a hash of the request URL,
/// query parameters, and optional POST body. Expired entries are treated as
/// cache misses and silently ignored.
///
/// # Atomic writes
///
/// Writes use a temporary file + rename pattern to prevent partial reads from
/// concurrent access.
#[derive(Clone, Debug)]
pub struct DiskCache {
    cache_dir: PathBuf,
    ttl: Duration,
}

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    ts: u64,
    body: String,
}

impl DiskCache {
    /// Create a cache storing entries in `cache_dir` with the given TTL.
    ///
    /// Creates the directory (and parents) if it doesn't exist.
    pub fn new(cache_dir: PathBuf, ttl: Duration) -> io::Result<Self> {
        std::fs::create_dir_all(&cache_dir)?;
        let cache = Self { cache_dir, ttl };
        cache.prune();
        Ok(cache)
    }

    /// Create a cache in the platform-standard cache directory.
    ///
    /// - Linux: `~/.cache/papers/requests`
    /// - macOS: `~/Library/Caches/papers/requests`
    /// - Windows: `{FOLDERID_LocalAppData}/papers/requests`
    ///
    /// Returns `Err` if no cache directory can be determined or created.
    pub fn default_location(ttl: Duration) -> io::Result<Self> {
        let base = dirs::cache_dir().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "no platform cache directory")
        })?;
        Self::new(base.join("papers").join("requests"), ttl)
    }

    /// Look up a cached response.
    ///
    /// Returns `None` on cache miss, expired entry, or any I/O / parse error.
    pub fn get(&self, url: &str, query: &[(&str, String)], body: Option<&str>) -> Option<String> {
        let key = cache_key(url, query, body);
        let path = self.cache_dir.join(format!("{key:016x}.json"));
        let data = std::fs::read_to_string(&path).ok()?;
        let entry: CacheEntry = serde_json::from_str(&data).ok()?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
        if now.saturating_sub(entry.ts) > self.ttl.as_secs() {
            return None;
        }
        Some(entry.body)
    }

    /// Store a response in the cache.
    ///
    /// Writes atomically via a `.tmp` file + rename. Errors are silently
    /// ignored — a failed cache write should never break a request.
    pub fn set(&self, url: &str, query: &[(&str, String)], body: Option<&str>, response: &str) {
        let _ = self.set_inner(url, query, body, response);
    }

    fn set_inner(
        &self,
        url: &str,
        query: &[(&str, String)],
        body: Option<&str>,
        response: &str,
    ) -> io::Result<()> {
        let key = cache_key(url, query, body);
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_secs();
        let entry = CacheEntry {
            ts,
            body: response.to_string(),
        };
        let json = serde_json::to_string(&entry).map_err(io::Error::other)?;
        let tmp_path = self.cache_dir.join(format!("{key:016x}.tmp"));
        let final_path = self.cache_dir.join(format!("{key:016x}.json"));
        std::fs::write(&tmp_path, json)?;
        std::fs::rename(&tmp_path, &final_path)?;
        Ok(())
    }

    /// Remove expired entries and leftover `.tmp` files from the cache directory.
    ///
    /// Called automatically on construction. Errors on individual files are
    /// silently ignored.
    pub fn prune(&self) {
        let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(d) => d.as_secs(),
            Err(_) => return,
        };
        let entries = match std::fs::read_dir(&self.cache_dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };
            // Clean up leftover .tmp files
            if name.ends_with(".tmp") {
                let _ = std::fs::remove_file(&path);
                continue;
            }
            // Only process our .json cache files
            if !name.ends_with(".json") {
                continue;
            }
            let data = match std::fs::read_to_string(&path) {
                Ok(d) => d,
                Err(_) => {
                    let _ = std::fs::remove_file(&path);
                    continue;
                }
            };
            let entry: CacheEntry = match serde_json::from_str(&data) {
                Ok(e) => e,
                Err(_) => {
                    let _ = std::fs::remove_file(&path);
                    continue;
                }
            };
            if now.saturating_sub(entry.ts) > self.ttl.as_secs() {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}

/// Compute a deterministic cache key from (url, sorted query pairs, optional body).
fn cache_key(url: &str, query: &[(&str, String)], body: Option<&str>) -> u64 {
    let mut sorted: Vec<(&str, &str)> = query.iter().map(|(k, v)| (*k, v.as_str())).collect();
    sorted.sort();
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    for (k, v) in &sorted {
        k.hash(&mut hasher);
        v.hash(&mut hasher);
    }
    if let Some(b) = body {
        b.hash(&mut hasher);
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    fn temp_cache(ttl_secs: u64) -> DiskCache {
        let dir = std::env::temp_dir()
            .join("papers-zotero-test-cache")
            .join(format!("{:x}", rand_u64()));
        DiskCache::new(dir, Duration::from_secs(ttl_secs)).unwrap()
    }

    fn rand_u64() -> u64 {
        let mut hasher = DefaultHasher::new();
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);
        std::thread::current().id().hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn key_is_deterministic() {
        let q = vec![("a", "1".into()), ("b", "2".into())];
        let k1 = cache_key("http://x", &q, None);
        let k2 = cache_key("http://x", &q, None);
        assert_eq!(k1, k2);
    }

    #[test]
    fn key_query_order_independent() {
        let q1 = vec![("b", "2".into()), ("a", "1".into())];
        let q2 = vec![("a", "1".into()), ("b", "2".into())];
        assert_eq!(
            cache_key("http://x", &q1, None),
            cache_key("http://x", &q2, None)
        );
    }

    #[test]
    fn set_get_roundtrip() {
        let cache = temp_cache(60);
        let q = vec![("k", "v".into())];
        cache.set("http://x", &q, None, "response body");
        let got = cache.get("http://x", &q, None);
        assert_eq!(got.as_deref(), Some("response body"));
    }

    #[test]
    fn missing_key_returns_none() {
        let cache = temp_cache(60);
        let q: Vec<(&str, String)> = vec![];
        assert!(cache.get("http://nonexistent", &q, None).is_none());
    }

    #[test]
    fn expired_entry_returns_none() {
        let cache = temp_cache(1);
        let q: Vec<(&str, String)> = vec![];
        cache.set("http://x", &q, None, "data");
        sleep(Duration::from_secs(2));
        assert!(cache.get("http://x", &q, None).is_none());
    }
}
