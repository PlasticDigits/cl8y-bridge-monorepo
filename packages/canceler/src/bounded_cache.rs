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

/// Bounded key→value cache with TTL and max-size eviction (C12: pending retry queue).
///
/// Like `BoundedHashCache` but stores a value alongside each 32-byte key.
/// Used for the pending approval retry queue to ensure approvals that returned
/// `Pending` are retried in subsequent poll cycles rather than dropped.
///
/// Memory bound: each entry is `size_of::<V>() + 32 + 16` bytes (key + Instant).
/// With `PendingApproval` (~200 bytes) and a 10 000 entry cap, worst-case is ~2.5 MB.
pub struct BoundedMapCache<V> {
    map: HashMap<[u8; 32], (V, Instant)>,
    max_size: usize,
    ttl: Duration,
}

impl<V> BoundedMapCache<V> {
    pub fn new(max_size: usize, ttl_secs: u64) -> Self {
        Self {
            map: HashMap::new(),
            max_size,
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    /// Insert or update an entry. Evicts expired/oldest entries if at capacity.
    /// If the key already exists, the value and timestamp are replaced.
    pub fn insert(&mut self, key: [u8; 32], value: V) {
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
        self.map.insert(key, (value, now));
    }

    pub fn remove(&mut self, key: &[u8; 32]) {
        self.map.remove(key);
    }

    /// Drain all entries, returning their values. The cache is empty afterwards.
    pub fn take_all(&mut self) -> Vec<V> {
        let now = Instant::now();
        self.map
            .drain()
            .filter(|(_, (_, t))| now.duration_since(*t) < self.ttl)
            .map(|(_, (v, _))| v)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }

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

    #[test]
    fn test_map_cache_insert_and_take_all() {
        let mut cache = BoundedMapCache::<u64>::new(10, 3600);
        assert!(cache.is_empty());
        cache.insert([1u8; 32], 100);
        cache.insert([2u8; 32], 200);
        assert_eq!(cache.len(), 2);

        let items = cache.take_all();
        assert_eq!(items.len(), 2);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_map_cache_evicts_oldest_when_full() {
        let mut cache = BoundedMapCache::<&str>::new(2, 3600);
        cache.insert([1u8; 32], "first");
        cache.insert([2u8; 32], "second");
        assert_eq!(cache.len(), 2);

        cache.insert([3u8; 32], "third");
        assert_eq!(cache.len(), 2, "should stay at capacity");
        let items = cache.take_all();
        let vals: Vec<&str> = items.into_iter().collect();
        assert!(vals.contains(&"second"));
        assert!(vals.contains(&"third"));
        assert!(!vals.contains(&"first"), "oldest should be evicted");
    }

    #[test]
    fn test_map_cache_ttl_eviction_on_take() {
        let mut cache = BoundedMapCache::<u64>::new(100, 1);
        cache.insert([1u8; 32], 42);
        assert_eq!(cache.len(), 1);
        sleep(Duration::from_secs(2));
        let items = cache.take_all();
        assert!(items.is_empty(), "expired entries filtered on take_all");
    }

    #[test]
    fn test_map_cache_remove() {
        let mut cache = BoundedMapCache::<u64>::new(10, 3600);
        cache.insert([1u8; 32], 42);
        assert_eq!(cache.len(), 1);
        cache.remove(&[1u8; 32]);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_map_cache_update_existing_key() {
        let mut cache = BoundedMapCache::<u64>::new(10, 3600);
        cache.insert([1u8; 32], 100);
        cache.insert([1u8; 32], 200);
        assert_eq!(cache.len(), 1, "duplicate key should not increase size");
        let items = cache.take_all();
        assert_eq!(items, vec![200]);
    }
}
