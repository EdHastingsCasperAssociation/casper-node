use borsh::{BorshDeserialize, BorshSerialize};

use super::{IterableMap, IterableMapKey};

/// An iterable set backed by a map.
pub struct IterableSet<V> {
    pub(crate) map: IterableMap<V, V>
}

impl<V: BorshSerialize + BorshDeserialize> IterableSet<V> {
    /// Creates an empty [IterableMap] with the given prefix.
    pub fn new<S: Into<String>>(prefix: S) -> Self {
        Self {
            map: IterableMap::new(prefix),
        }
    }

    /// Inserts a value into the set.
    pub fn insert(&mut self, value: V) {
        let hash = value.compute_key_hash();
        self.map.insert_impl(hash, value);
    }

    /// Removes a value from the set.
    /// 
    /// Has a worst-case runtime of O(n).
    pub fn remove(&mut self, value: &V) {
        self.map.remove(value);
    }

    /// Returns true if the set contains a value.
    pub fn contains(&self, value: &V) -> bool {
        self.map.get(value).is_some()
    }

    /// Creates an iterator visiting all the values in arbitrary order.
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = V> + 'a {
        self.map.iter().map(|(_, value)| value)
    }

    // Returns true if the set contains no elements.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Clears the set, removing all values.
    pub fn clear(&mut self) {
        self.map.clear();
    }
}