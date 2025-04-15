use core::marker::PhantomData;

use borsh::{BorshDeserialize, BorshSerialize};
use bytes::BufMut;
use casper_executor_wasm_common::keyspace::Keyspace;
use const_fnv1a_hash::fnv1a_hash_128;

use crate::casper::{self, read_into_vec};

/// Internal key representation after hashing. Currently [`u128`].
pub type IterableMapKeyRepr = u128;

/// Trait for types that can be used as keys in [IterableMap].
/// Must produce a deterministic [IterableMapKeyRepr] prefix via hashing.
/// 
/// A blanket implementation is provided for all types that implement
/// [BorshSerialize].
pub trait IterableMapKey {
    fn to_key_prefix(&self) -> IterableMapKeyRepr;
}

impl<K: BorshSerialize> IterableMapKey for K {
    fn to_key_prefix(&self) -> IterableMapKeyRepr {
        let mut bytes = Vec::new();
        self.serialize(&mut bytes).unwrap();
        fnv1a_hash_128(&bytes, None)
    }
}

/// A singly-linked map. Each entry at key `K_n` stores `(V, K_{n-1})`,
/// where `V` is the value and `K_{n-1}` is the key of the previous entry.
/// 
/// This creates a constant spatial overhead; every entry stores a pointer
/// to the one inserted before it.
/// 
/// Enables iteration without a guaranteed ordering; updating an existing
/// key does not affect position.
/// 
/// Supports full traversal, typically in reverse-insertion order.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
#[borsh(crate = "crate::serializers::borsh")]
pub struct IterableMap<K, V> {
    pub(crate) prefix: String,

    // Keys are hashed to u128 internally, but K is preserved to enforce type safety.
    // While this map could accept arbitrary u128 keys, requiring a concrete K prevents
    // misuse and clarifies intent at the type level.
    pub(crate) head_key_hash: Option<IterableMapKeyRepr>,
    _marker: PhantomData<(K, V)>
}

/// Single entry in `IterableMap`. Stores the value and the hash of the previous entry's key.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
#[borsh(crate = "crate::serializers::borsh")]
pub struct IterableMapEntry<V> {
    pub(crate) value: V,
    pub(crate) previous: Option<IterableMapKeyRepr>,
}

impl<K, V> IterableMap<K, V>
where
    K: IterableMapKey,
    V: BorshSerialize + BorshDeserialize,
{
    /// Creates an empty [IterableMap] with the given prefix.
    pub fn new<S: Into<String>>(prefix: S) -> Self {
        Self {
            prefix: prefix.into(),
            head_key_hash: None,
            _marker: Default::default(),
        }
    }

    /// Inserts a key-value pair into the map.
    /// 
    /// If the map did not have this key present, None is returned.
    /// 
    /// If the map did have this key present, the value is updated, and the old value is returned.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let prefix = self.create_prefix_from_key(&key);
        let context_key = Keyspace::Context(&prefix);

        // Either overwrite an existing entry, or create a new one.
        let (entry_to_write, previous) = match self.get_entry(context_key) {
            Some(mut entry) => {
                let old_value = entry.value;
                entry.value = value;
                (entry, Some(old_value))
            },
            None => {
                let entry = IterableMapEntry {
                    value,
                    previous: self.head_key_hash
                };

                // Additionally, since this is a new entry, we need to update the head
                self.head_key_hash = Some(key.to_key_prefix());

                (entry, None)
            }
        };

        // Write the entry and return previous value if it exists
        let mut entry_bytes = Vec::new();
        entry_to_write.serialize(&mut entry_bytes).unwrap();
        casper::write(context_key, &entry_bytes).unwrap();

        return previous;
    }

    /// Returns a value corresponding to the key.
    pub fn get(&self, key: &K) -> Option<V> {
        let prefix = self.create_prefix_from_key(&key);
        let context_key = Keyspace::Context(&prefix);

        self.get_entry(context_key).map(|entry| entry.value)
    }

    /// Removes a key from the map. Returns the associated value if the key exists.
    /// 
    /// Has a worst-case runtime of O(n).
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let to_remove_hash = key.to_key_prefix();
        let to_remove_prefix = self.create_prefix_from_hash(to_remove_hash);
        let Some(to_remove_entry) = self.get_entry(Keyspace::Context(&to_remove_prefix)) else {
            // The entry to remove doesn't exist.
            return None;
        };

        // Purge the entry from global state
        purge_at_key(Keyspace::Context(&to_remove_prefix));
        
        // Edge case when removing head
        if self.head_key_hash == Some(to_remove_hash) {
            self.head_key_hash = to_remove_entry.previous;
            return Some(to_remove_entry.value);
        }

        // Scan the map, find entry to remove, join adjacent entries
        let mut current_hash = self.head_key_hash;
        while let Some(key) = current_hash {
            let current_prefix = self.create_prefix_from_hash(key);
            let current_context_key = Keyspace::Context(&current_prefix);
            let mut current_entry = self.get_entry(current_context_key).unwrap();
            
            // If there is no previous entry, then we've finished iterating
            let Some(next_hash) = current_entry.previous else {
                break;
            };

            // If the next entry is the one to be removed, repoint the current
            // one to the one preceeding the one to remove.
            if next_hash == to_remove_hash {
                current_entry.previous = to_remove_entry.previous;

                // Re-write the current entry
                let mut entry_bytes = Vec::new();
                current_entry.serialize(&mut entry_bytes).unwrap();
                casper::write(current_context_key, &entry_bytes).unwrap();

                return Some(to_remove_entry.value);
            }

            // Advance backwards
            current_hash = current_entry.previous;
        }

        None
    }

    /// Returns an iterator over the entries in the map.
    /// 
    /// Traverses entries in reverse-insertion order.
    /// Each item is a tuple of the hashed key and the value.
    ///
    /// Note: the original key type `K` is not recoverable during iteration.
    pub fn iter(&self) -> IterableMapIter<K, V>
    where
        V: BorshDeserialize,
    {
        IterableMapIter {
            prefix: &self.prefix,
            current: self.head_key_hash,
            _marker: PhantomData,
        }
    }

    fn get_entry(&self, keyspace: Keyspace) -> Option<IterableMapEntry<V>> {
        read_into_vec(keyspace).map(|vec| borsh::from_slice(&vec).ok()).flatten()
    }

    fn create_prefix_from_key(&self, key: &K) -> Vec<u8> {
        let hash = key.to_key_prefix();
        self.create_prefix_from_hash(hash)
    }

    fn create_prefix_from_hash(&self, hash: IterableMapKeyRepr) -> Vec<u8> {
        let mut context_key = Vec::new();
        context_key.extend(self.prefix.as_bytes());
        context_key.extend("_".as_bytes());
        context_key.put_u128_le(hash);
        context_key
    }
}

// TODO: Rn we just overwrite with zeroes ¯\_(ツ)_/¯
// This is placeholder, and is to be removed when the appropriate functionality merges.
fn purge_at_key(key: Keyspace) {
    casper::write(key, &[0]).unwrap();
}

/// Iterator over entries in an [`IterableMap`].
///
/// Traverses the map in reverse-insertion order, following the internal
/// linked structure via hashed key references [`u128`].
///
/// Yields a tuple ([`IterableMapKeyRepr`], V), where the key is the hashed
/// representation of the original key. The original key type `K` is not recoverable.
///
/// Each iteration step deserializes a single entry from storage.
/// 
/// This iterator performs no allocation beyond internal buffers,
/// and deserialization errors are treated as iteration termination.
pub struct IterableMapIter<'a, K, V> {
    prefix: &'a str,
    current: Option<IterableMapKeyRepr>,
    _marker: PhantomData<(K, V)>,
}

impl<'a, K, V> IntoIterator for &'a IterableMap<K, V>
where
    K: IterableMapKey,
    V: BorshDeserialize,
{
    type Item = (IterableMapKeyRepr, V);
    type IntoIter = IterableMapIter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        IterableMapIter {
            prefix: &self.prefix,
            current: self.head_key_hash,
            _marker: PhantomData,
        }
    }
}

impl<'a, K, V> Iterator for IterableMapIter<'a, K, V>
where
    V: BorshDeserialize,
{
    type Item = (IterableMapKeyRepr, V);

    fn next(&mut self) -> Option<Self::Item> {
        let current_hash = self.current?;
        let mut key_bytes = Vec::new();
        key_bytes.extend(self.prefix.as_bytes());
        key_bytes.extend("_".as_bytes());
        key_bytes.put_u128_le(current_hash);

        let context_key = Keyspace::Context(&key_bytes);

        let Some(entry) = read_into_vec(context_key)
            .map(|vec| borsh::from_slice::<IterableMapEntry<V>>(&vec).unwrap()) 
        else {
            return None;
        };

        self.current = entry.previous;
        Some((current_hash, entry.value))
    }
}

#[cfg(test)]
mod tests {
    use crate::casper::native::dispatch;
    use super::*;

    #[test]
    fn insert_and_get() {
        dispatch(|| {
            let mut map = IterableMap::<u64, String>::new("test_map");

            assert_eq!(map.get(&1), None);

            map.insert(1, "a".to_string());
            assert_eq!(map.get(&1), Some("a".to_string()));

            map.insert(2, "b".to_string());
            assert_eq!(map.get(&2), Some("b".to_string()));
        }).unwrap();
    }

    #[test]
    fn overwrite_existing_key() {
        dispatch(|| {
            let mut map = IterableMap::<u64, String>::new("test_map");

            assert_eq!(map.insert(1, "a".to_string()), None);
            assert_eq!(map.insert(1, "b".to_string()), Some("a".to_string()));
            assert_eq!(map.get(&1), Some("b".to_string()));
        }).unwrap();
    }

    #[test]
    fn remove_head_entry() {
        dispatch(|| {
            let mut map = IterableMap::<u64, String>::new("test_map");

            map.insert(1, "a".to_string());
            map.insert(2, "b".to_string());

            assert_eq!(map.remove(&2), Some("b".to_string()));
            assert_eq!(map.get(&2), None);
            assert_eq!(map.get(&1), Some("a".to_string()));
        }).unwrap();
    }

    #[test]
    fn remove_middle_entry() {
        dispatch(|| {
            let mut map = IterableMap::<u64, String>::new("test_map");

            map.insert(1, "a".to_string());
            map.insert(2, "b".to_string());
            map.insert(3, "c".to_string());

            assert_eq!(map.remove(&2), Some("b".to_string()));
            assert_eq!(map.get(&2), None);
            assert_eq!(map.get(&1), Some("a".to_string()));
            assert_eq!(map.get(&3), Some("c".to_string()));
        }).unwrap();
    }

    #[test]
    fn remove_nonexistent_key_does_nothing() {
        dispatch(|| {
            let mut map = IterableMap::<u64, String>::new("test_map");

            map.insert(1, "a".to_string());

            assert_eq!(map.remove(&999), None);
            assert_eq!(map.get(&1), Some("a".to_string()));
        }).unwrap();
    }

    #[test]
    fn iterates_all_entries_in_reverse_insertion_order() {
        dispatch(|| {
            let mut map = IterableMap::<u64, String>::new("test_map");

            map.insert(1, "a".to_string());
            map.insert(2, "b".to_string());
            map.insert(3, "c".to_string());

            let values: Vec<_> = map.iter().map(|(_, v)| v).collect();
            assert_eq!(values, vec![
                "c".to_string(),
                "b".to_string(),
                "a".to_string(),
            ]);
        }).unwrap();
    }

    #[test]
    fn iteration_skips_deleted_entries() {
        dispatch(|| {
            let mut map = IterableMap::<u64, String>::new("test_map");

            map.insert(1, "a".to_string());
            map.insert(2, "b".to_string());
            map.insert(3, "c".to_string());

            map.remove(&2);

            let values: Vec<_> = map.iter().map(|(_, v)| v).collect();
            assert_eq!(values, vec![
                "c".to_string(),
                "a".to_string(),
            ]);
        }).unwrap();
    }

    #[test]
    fn empty_map_behaves_sanely() {
        dispatch(|| {
            let mut map = IterableMap::<u64, String>::new("test_map");

            assert_eq!(map.get(&1), None);
            assert_eq!(map.remove(&1), None);
            assert_eq!(map.iter().count(), 0);
        }).unwrap();
    }

    #[test]
    fn separate_maps_do_not_conflict() {
        dispatch(|| {
            let mut map1 = IterableMap::<u64, String>::new("map1");
            let mut map2 = IterableMap::<u64, String>::new("map2");

            map1.insert(1, "a".to_string());
            map2.insert(1, "b".to_string());

            assert_eq!(map1.get(&1), Some("a".to_string()));
            assert_eq!(map2.get(&1), Some("b".to_string()));
        }).unwrap();
    }

    #[test]
    fn insert_same_value_under_different_keys() {
        dispatch(|| {
            let mut map = IterableMap::<u64, String>::new("test_map");

            map.insert(1, "shared".to_string());
            map.insert(2, "shared".to_string());

            assert_eq!(map.get(&1), Some("shared".to_string()));
            assert_eq!(map.get(&2), Some("shared".to_string()));
        }).unwrap();
    }
}