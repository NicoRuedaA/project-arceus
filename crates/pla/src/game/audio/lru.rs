//! Least-recently-used cache bounded by a byte budget (the decoded audio is about 5 GB; only
//! what is being played stays in memory).

use std::collections::HashMap;
use std::hash::Hash;

/// LRU map from `K` to `V`, each entry weighing `bytes`. Inserting evicts the least recently
/// used entries until the total fits the budget; an entry bigger than the whole budget is still
/// kept (alone) so a long music track can play.
#[derive(Debug)]
pub struct LruCache<K, V> {
    budget: usize,
    used: usize,
    clock: u64,
    map: HashMap<K, (V, usize, u64)>,
}

impl<K: Eq + Hash + Clone, V> LruCache<K, V> {
    pub fn new(budget_bytes: usize) -> Self {
        Self { budget: budget_bytes, used: 0, clock: 0, map: HashMap::new() }
    }

    /// Bytes held.
    pub fn used(&self) -> usize {
        self.used
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn contains(&self, k: &K) -> bool {
        self.map.contains_key(k)
    }

    /// Value of `k`, marking it as the most recently used.
    pub fn get(&mut self, k: &K) -> Option<&V> {
        self.clock += 1;
        let now = self.clock;
        self.map.get_mut(k).map(|e| {
            e.2 = now;
            &e.0
        })
    }

    /// Insert (or replace) `k`. Returns the keys evicted to make room.
    pub fn insert(&mut self, k: K, v: V, bytes: usize) -> Vec<K> {
        if let Some((_, old, _)) = self.map.remove(&k) {
            self.used -= old;
        }
        let mut evicted = Vec::new();
        while !self.map.is_empty() && self.used + bytes > self.budget {
            let oldest = self
                .map
                .iter()
                .min_by_key(|(_, (_, _, t))| *t)
                .map(|(k, _)| k.clone())
                .expect("non-empty map");
            if let Some((_, b, _)) = self.map.remove(&oldest) {
                self.used -= b;
            }
            evicted.push(oldest);
        }
        self.clock += 1;
        self.used += bytes;
        self.map.insert(k, (v, bytes, self.clock));
        evicted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_least_recently_used_first() {
        let mut c = LruCache::new(100);
        assert!(c.insert("a", 1, 40).is_empty());
        assert!(c.insert("b", 2, 40).is_empty());
        assert_eq!(c.get(&"a"), Some(&1)); // a is now newer than b
        assert_eq!(c.insert("c", 3, 40), vec!["b"]);
        assert!(c.contains(&"a") && c.contains(&"c") && !c.contains(&"b"));
        assert_eq!(c.used(), 80);
    }

    #[test]
    fn evicts_several_for_a_big_entry_and_keeps_an_oversized_one_alone() {
        let mut c = LruCache::new(100);
        c.insert(1, (), 30);
        c.insert(2, (), 30);
        c.insert(3, (), 30);
        let mut ev = c.insert(4, (), 70);
        ev.sort();
        assert_eq!(ev, vec![1, 2], "evicts oldest first, only until it fits");
        assert_eq!(c.used(), 100);
        let mut ev = c.insert(5, (), 500);
        ev.sort();
        assert_eq!(ev, vec![3, 4]);
        assert_eq!((c.len(), c.used()), (1, 500));
        assert_eq!(c.insert(6, (), 10), vec![5]);
    }

    #[test]
    fn replacing_a_key_updates_its_weight() {
        let mut c = LruCache::new(100);
        c.insert("a", 1, 60);
        assert!(c.insert("a", 2, 30).is_empty());
        assert_eq!((c.len(), c.used()), (1, 30));
        assert_eq!(c.get(&"a"), Some(&2));
        assert_eq!(c.get(&"zz"), None);
    }
}
