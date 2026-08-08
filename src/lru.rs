//! Cleanroom Rust port of upstream Go source file: `internal/lru/lru.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! A simple, deterministic LRU cache. The upstream Go implementation uses a
//! hash map plus a doubly-linked list; this port uses a `Vec` as the eviction
//! list (most recently used at the front) with a `HashMap` of key-to-index
//! entries, which preserves the exact API surface and semantics with
//! O(n) operations.
//! </public-docs>

use std::collections::HashMap;

/// LRU is a fixed-size least-recently-used cache with a generic key and
/// value type.
#[derive(Debug, Clone)]
pub struct Lru<K, V> {
    size: usize,
    items: HashMap<K, usize>,
    evict: Vec<(K, V)>,
}

/// New creates a new LRU cache with the given size.
///
/// It panics if the size is negative.
pub fn new<K, V>(size: i64) -> Lru<K, V>
where
    K: Eq + std::hash::Hash + Clone,
{
    if size < 0 {
        panic!("lru: negative size given: {size}");
    }
    Lru {
        size: size as usize,
        items: HashMap::new(),
        evict: Vec::new(),
    }
}

impl<K, V> Lru<K, V>
where
    K: Eq + std::hash::Hash + Clone,
{
    /// Returns the length of the cache.
    pub fn len(&self) -> usize {
        self.evict.len()
    }

    /// Returns whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.evict.is_empty()
    }

    /// Get returns the value associated with the key, moving it to the front
    /// of the eviction list. Returns None if the key is not present.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        let idx = *self.items.get(key)?;

        let entry = self.evict.remove(idx);
        // Decrement the stored index of every entry after the removed
        // position, then bump every entry when inserting at the front.
        for j in self.items.values_mut() {
            if *j > idx {
                *j -= 1;
            }
        }
        self.evict.insert(0, entry);
        for j in self.items.values_mut() {
            *j += 1;
        }

        self.items.insert(self.evict[0].0.clone(), 0);
        Some(&self.evict[0].1)
    }

    /// Add inserts the key/value pair into the cache, moving an existing
    /// entry to the front or evicting the least recently used entry when the
    /// cache is full. Returns true if an entry was evicted.
    pub fn add(&mut self, key: K, value: V) -> bool {
        if let Some(idx) = self.items.get(&key).copied() {
            self.evict.remove(idx);
            for j in self.items.values_mut() {
                if *j > idx {
                    *j -= 1;
                }
            }
            self.evict.insert(0, (key, value));
            for j in self.items.values_mut() {
                *j += 1;
            }
            self.items.insert(self.evict[0].0.clone(), 0);
            return false;
        }

        self.evict.insert(0, (key, value));
        for j in self.items.values_mut() {
            *j += 1;
        }
        self.items.insert(self.evict[0].0.clone(), 0);

        if self.evict.len() <= self.size {
            return false;
        }

        if let Some((oldest, _)) = self.evict.pop() {
            self.items.remove(&oldest);
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lru() {
        const SIZE: i64 = 20;

        let mut cache: Lru<i64, String> = new(SIZE);

        for i in 0..SIZE {
            let v = i.to_string();

            assert!(!cache.add(i, v.clone()), "value evicted before size limit at {i}");

            let got = cache.get(&i);
            assert!(got.is_some(), "value not found at key {i}");
            assert_eq!(*got.unwrap(), v, "value at key {i} not equal");
        }

        let v = SIZE.to_string();
        assert!(cache.add(SIZE, v.clone()), "value not evicted after limit");
        assert!(cache.get(&0).is_none(), "value at key 0 not evicted");

        for i in 1..=SIZE {
            let got = cache.get(&i);
            assert!(got.is_some(), "value not found at key {i}");
            assert_eq!(*got.unwrap(), i.to_string(), "value at key {i} not equal");
        }
    }

    #[test]
    fn test_negative_size_panics() {
        let result = std::panic::catch_unwind(|| new::<i64, i64>(-1));
        assert!(result.is_err());
    }
}
