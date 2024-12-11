//! Thread-safe key/value cache.

use std::clone;
use std::collections::hash_map::{DefaultHasher, Entry, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Condvar, Mutex, RwLock};
const NUM_PARTITIONS: usize = 32;
/// Cache that remembers the result for each key.
#[derive(Debug)]
pub struct Cache<K, V> {
    // todo! This is an example cache type. Build your own cache type that satisfies the
    // specification for `get_or_insert_with`.
    inner: Vec<RwLock<HashMap<K, V>>>,
}

impl<K, V> Default for Cache<K, V> {
    fn default() -> Self {
        let mut inner = Vec::with_capacity(NUM_PARTITIONS);
        for i in 0..NUM_PARTITIONS {
            inner.push(RwLock::new(HashMap::new()));
        }
        return Cache { inner: inner };
    }
}

impl<K: Eq + Hash + Clone, V: Clone> Cache<K, V> {
    /// Retrieve the value or insert a new one created by `f`.
    ///
    /// An invocation to this function should not block another invocation with a different key. For
    /// example, if a thread calls `get_or_insert_with(key1, f1)` and another thread calls
    /// `get_or_insert_with(key2, f2)` (`key1≠key2`, `key1,key2∉cache`) concurrently, `f1` and `f2`
    /// should run concurrently.
    ///
    /// On the other hand, since `f` may consume a lot of resource (= money), it's undesirable to
    /// duplicate the work. That is, `f` should be run only once for each key. Specifically, even
    /// for concurrent invocations of `get_or_insert_with(key, f)`, `f` is called only once per key.
    ///
    /// Hint: the [`Entry`] API may be useful in implementing this function.
    ///
    /// [`Entry`]: https://doc.rust-lang.org/stable/std/collections/hash_map/struct.HashMap.html#method.entry
    pub fn get_or_insert_with<F: FnOnce(K) -> V>(&self, key: K, f: F) -> V {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let partition = (hasher.finish() % 32) as usize;
        {
            let guard = self.inner[partition].read().unwrap();
            match guard.get(&key) {
                Some(v) => {
                    return v.clone();
                }
                None => {}
            }
        }
        let mut guard = self.inner[partition].write().unwrap();
        let v = guard.entry(key.clone()).or_insert_with(move || f(key));
        return v.clone();
    }
}
