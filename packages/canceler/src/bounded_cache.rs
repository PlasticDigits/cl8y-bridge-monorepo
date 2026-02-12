//! Bounded hash cache with TTL and max-size eviction (C3: security review)
//!
//! Replaces unbounded `HashSet` storage for verified/cancelled hashes to prevent
//! unbounded memory growth under long runtimes or adversarial event volume.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Bounded cache for 32-byte hashes with TTL and capacity limits.
///
/// - **Max capacity:** Configurable; when full, oldest entry is evicted on insert.
/// - **TTL:** Entries older than TTL are evicted before insertion when at capacity.
/// - **Eviction:** On insert when at capacity, first evict expired entries, then
///   evict the oldest remaining entry by insertion time.
pub struct BoundedHashCache {
    /// Hash -> insertion timestamp
    map: HashMap<[u8; 32], Instant>,
    max_size: usize,
    ttl: Duration,
}

impl BoundedHashCache {
    /// Create a new bounded cache.
    ///
    /// - `max_size`: Maximum number of entries; oldest evicted when exceeded.
    /// - `ttl_secs`: Entries older than this are eligible for eviction.
    pub fn new(max_size: usize, ttl_secs: u64) -> Self {
        Self {
            map: HashMap::new(),
            max_size,
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    /// Returns true if the hash is present and not expired.
    pub fn contains(&self, hash: &[u8; 32]) -> bool {
        self.map.get(hash).is_some_and(|&t| t.elapsed() < self.ttl)
    }

    /// Insert a hash. Evicts oldest/expired entries if at capacity.
    pub fn insert(&mut self, hash: [u8; 32]) {
        let now = Instant::now();

        // Evict expired entries first
        self.map
            .retain(|_, &mut t| now.duration_since(t) < self.ttl);

        // If still at capacity, evict oldest
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

    /// Current number of entries.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Returns (current len, max_size) for capacity percentage checks.
    pub fn capacity_info(&self) -> (usize, usize) {
        (self.map.len(), self.max_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_bounded_cache_insert_and_contains() {
        let mut cache = BoundedHashCache::new(10, 3600);
        let hash = [1u8; 32];
        assert!(!cache.contains(&hash));
        cache.insert(hash);
        assert!(cache.contains(&hash));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_bounded_cache_evicts_oldest_when_full() {
        let mut cache = BoundedHashCache::new(3, 3600);
        let h1 = [1u8; 32];
        let h2 = [2u8; 32];
        let h3 = [3u8; 32];
        let h4 = [4u8; 32];

        cache.insert(h1);
        cache.insert(h2);
        cache.insert(h3);
        assert!(cache.contains(&h1));
        assert!(cache.contains(&h2));
        assert!(cache.contains(&h3));
        assert_eq!(cache.len(), 3);

        cache.insert(h4);
        assert!(!cache.contains(&h1), "oldest should be evicted");
        assert!(cache.contains(&h2));
        assert!(cache.contains(&h3));
        assert!(cache.contains(&h4));
        assert_eq!(cache.len(), 3);
    }

    #[test]
    fn test_bounded_cache_ttl_eviction() {
        let mut cache = BoundedHashCache::new(100, 1); // 1 second TTL
        let hash = [42u8; 32];
        cache.insert(hash);
        assert!(cache.contains(&hash));
        sleep(Duration::from_secs(2));
        assert!(!cache.contains(&hash), "expired entry should not be found");
        // Insert new entry to trigger retain(); expired entries are evicted
        cache.insert([99u8; 32]);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_bounded_cache_clear() {
        let mut cache = BoundedHashCache::new(10, 3600);
        cache.insert([1u8; 32]);
        cache.insert([2u8; 32]);
        assert_eq!(cache.len(), 2);
        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(!cache.contains(&[1u8; 32]));
        assert!(!cache.contains(&[2u8; 32]));
    }

    #[test]
    fn test_bounded_cache_capacity_info() {
        let mut cache = BoundedHashCache::new(50, 3600);
        cache.insert([1u8; 32]);
        let (len, max) = cache.capacity_info();
        assert_eq!(len, 1);
        assert_eq!(max, 50);
    }
}
