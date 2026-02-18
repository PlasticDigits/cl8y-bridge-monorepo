//! Bounded caches with TTL and max-size eviction for the operator.
//!
//! Prevents unbounded memory growth for `approved_hashes` and `pending_executions`
//! HashMaps under long runtimes or adversarial event volume.
//!
//! ## Recommended RAM
//!
//! - Each hash entry is ~40 bytes (32-byte key + 8-byte Instant), so 100k entries ≈ 4 MB.
//! - Each pending execution entry is ~200 bytes, so 50k entries ≈ 10 MB.
//! - **Minimum recommended RAM: 512 MB** for default cache sizes.
//! - Scale linearly for larger caches (e.g., 1 GB for 2x defaults).

use std::collections::HashMap;
use std::env;
use std::time::{Duration, Instant};

const DEFAULT_APPROVED_HASH_CACHE_SIZE: usize = 100_000;
const DEFAULT_PENDING_EXECUTION_CACHE_SIZE: usize = 50_000;
const DEFAULT_HASH_CACHE_TTL_SECS: u64 = 86_400; // 24 hours

/// Read cache configuration from environment variables with defaults.
pub struct CacheConfig {
    pub approved_hash_size: usize,
    pub pending_execution_size: usize,
    pub ttl_secs: u64,
}

impl CacheConfig {
    pub fn from_env() -> Self {
        Self {
            approved_hash_size: env::var("APPROVED_HASH_CACHE_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_APPROVED_HASH_CACHE_SIZE),
            pending_execution_size: env::var("PENDING_EXECUTION_CACHE_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_PENDING_EXECUTION_CACHE_SIZE),
            ttl_secs: env::var("HASH_CACHE_TTL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_HASH_CACHE_TTL_SECS),
        }
    }
}

/// Bounded cache for 32-byte hashes with TTL and capacity limits.
///
/// Used for `approved_hashes` tracking. Identical to the canceler's BoundedHashCache
/// but with larger default sizes since the operator has more RAM.
///
/// - **Max capacity:** Configurable; when full, oldest entry is evicted on insert.
/// - **TTL:** Entries older than TTL are evicted before insertion when at capacity.
pub struct BoundedHashCache {
    map: HashMap<[u8; 32], Instant>,
    max_size: usize,
    ttl: Duration,
}

impl BoundedHashCache {
    pub fn new(max_size: usize, ttl_secs: u64) -> Self {
        Self {
            map: HashMap::new(),
            max_size,
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    pub fn contains_key(&self, hash: &[u8; 32]) -> bool {
        self.map.get(hash).is_some_and(|&t| t.elapsed() < self.ttl)
    }

    pub fn insert(&mut self, hash: [u8; 32]) {
        let now = Instant::now();
        self.map
            .retain(|_, &mut t| now.duration_since(t) < self.ttl);
        while self.map.len() >= self.max_size && !self.map.is_empty() {
            let oldest = self.map.iter().min_by_key(|(_, t)| *t).map(|(h, _)| *h);
            if let Some(h) = oldest {
                self.map.remove(&h);
            } else {
                break;
            }
        }
        self.map.insert(hash, now);
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.map.len()
    }
}

/// Bounded cache for pending executions with TTL and capacity limits.
///
/// Wraps a HashMap of `[u8; 32] → T` with max-size and TTL eviction.
pub struct BoundedPendingCache<T> {
    map: HashMap<[u8; 32], (T, Instant)>,
    max_size: usize,
    ttl: Duration,
}

impl<T> BoundedPendingCache<T> {
    pub fn new(max_size: usize, ttl_secs: u64) -> Self {
        Self {
            map: HashMap::new(),
            max_size,
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    #[allow(dead_code)]
    pub fn get(&self, hash: &[u8; 32]) -> Option<&T> {
        self.map
            .get(hash)
            .filter(|(_, t)| t.elapsed() < self.ttl)
            .map(|(v, _)| v)
    }

    pub fn insert(&mut self, hash: [u8; 32], value: T) {
        let now = Instant::now();
        self.map
            .retain(|_, (_, t)| now.duration_since(*t) < self.ttl);
        while self.map.len() >= self.max_size && !self.map.is_empty() {
            let oldest = self
                .map
                .iter()
                .min_by_key(|(_, (_, t))| *t)
                .map(|(h, _)| *h);
            if let Some(h) = oldest {
                self.map.remove(&h);
            } else {
                break;
            }
        }
        self.map.insert(hash, (value, now));
    }

    pub fn remove(&mut self, hash: &[u8; 32]) -> Option<T> {
        self.map.remove(hash).map(|(v, _)| v)
    }

    /// Iterate over entries (hash, value) for processing.
    pub fn iter(&self) -> impl Iterator<Item = (&[u8; 32], &T)> {
        self.map.iter().map(|(h, (v, _))| (h, v))
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_cache_insert_and_contains() {
        let mut cache = BoundedHashCache::new(10, 3600);
        let hash = [1u8; 32];
        assert!(!cache.contains_key(&hash));
        cache.insert(hash);
        assert!(cache.contains_key(&hash));
    }

    #[test]
    fn test_hash_cache_evicts_oldest() {
        let mut cache = BoundedHashCache::new(3, 3600);
        cache.insert([1u8; 32]);
        cache.insert([2u8; 32]);
        cache.insert([3u8; 32]);
        cache.insert([4u8; 32]);
        assert!(!cache.contains_key(&[1u8; 32]));
        assert!(cache.contains_key(&[4u8; 32]));
        assert_eq!(cache.len(), 3);
    }

    #[test]
    fn test_pending_cache_insert_and_get() {
        let mut cache = BoundedPendingCache::new(10, 3600);
        let hash = [1u8; 32];
        cache.insert(hash, "hello");
        assert_eq!(cache.get(&hash), Some(&"hello"));
    }

    #[test]
    fn test_pending_cache_evicts_oldest() {
        let mut cache = BoundedPendingCache::new(2, 3600);
        cache.insert([1u8; 32], "a");
        cache.insert([2u8; 32], "b");
        cache.insert([3u8; 32], "c");
        assert!(cache.get(&[1u8; 32]).is_none());
        assert_eq!(cache.get(&[3u8; 32]), Some(&"c"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_pending_cache_remove() {
        let mut cache = BoundedPendingCache::new(10, 3600);
        cache.insert([1u8; 32], 42);
        assert_eq!(cache.remove(&[1u8; 32]), Some(42));
        assert!(cache.get(&[1u8; 32]).is_none());
    }
}
