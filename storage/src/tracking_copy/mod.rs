//! This module defines the `TrackingCopy` - a utility that caches operations on the state, so that
//! the underlying state remains unmodified, but it can be interacted with as if the modifications
//! were applied on it.
mod byte_size;
mod error;
mod ext;
mod ext_entity;
mod meter;
#[cfg(test)]
mod tests;

use std::{
    borrow::Borrow,
    collections::{BTreeMap, BTreeSet, HashSet, VecDeque},
    convert::{From, TryInto},
    fmt::Debug,
    sync::Arc,
};

use linked_hash_map::LinkedHashMap;
use thiserror::Error;
use tracing::error;

use crate::{
    global_state::{
        error::Error as GlobalStateError, state::StateReader,
        trie_store::operations::compute_state_hash, DEFAULT_MAX_QUERY_DEPTH,
    },
    KeyPrefix,
};
use casper_types::{
    addressable_entity::NamedKeyAddr,
    bytesrepr::{self, ToBytes},
    contract_messages::{Message, Messages},
    contracts::NamedKeys,
    execution::{Effects, TransformError, TransformInstruction, TransformKindV2, TransformV2},
    global_state::TrieMerkleProof,
    handle_stored_dictionary_value, BlockGlobalAddr, CLType, CLValue, CLValueError, Digest, Key,
    KeyTag, StoredValue, StoredValueTypeMismatch, U512,
};

use self::meter::{heap_meter::HeapSize, Meter};
pub use self::{
    error::Error as TrackingCopyError,
    ext::TrackingCopyExt,
    ext_entity::{FeesPurseHandling, TrackingCopyEntityExt},
};

/// Result of a query on a `TrackingCopy`.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum TrackingCopyQueryResult {
    /// Invalid state root hash.
    RootNotFound,
    /// The value wasn't found.
    ValueNotFound(String),
    /// A circular reference was found in the state while traversing it.
    CircularReference(String),
    /// The query reached the depth limit.
    DepthLimit {
        /// The depth reached.
        depth: u64,
    },
    /// The query was successful.
    Success {
        /// The value read from the state.
        value: StoredValue,
        /// Merkle proofs for the value.
        proofs: Vec<TrieMerkleProof<Key, StoredValue>>,
    },
}

impl TrackingCopyQueryResult {
    /// Is this a successful query?
    pub fn is_success(&self) -> bool {
        matches!(self, TrackingCopyQueryResult::Success { .. })
    }

    /// As result.
    pub fn into_result(self) -> Result<StoredValue, TrackingCopyError> {
        match self {
            TrackingCopyQueryResult::RootNotFound => {
                Err(TrackingCopyError::Storage(Error::RootNotFound))
            }
            TrackingCopyQueryResult::ValueNotFound(msg) => {
                Err(TrackingCopyError::ValueNotFound(msg))
            }
            TrackingCopyQueryResult::CircularReference(msg) => {
                Err(TrackingCopyError::CircularReference(msg))
            }
            TrackingCopyQueryResult::DepthLimit { depth } => {
                Err(TrackingCopyError::QueryDepthLimit { depth })
            }
            TrackingCopyQueryResult::Success { value, .. } => Ok(value),
        }
    }
}

/// Struct containing state relating to a given query.
struct Query {
    /// The key from where the search starts.
    base_key: Key,
    /// A collection of normalized keys which have been visited during the search.
    visited_keys: HashSet<Key>,
    /// The key currently being processed.
    current_key: Key,
    /// Path components which have not yet been followed, held in the same order in which they were
    /// provided to the `query()` call.
    unvisited_names: VecDeque<String>,
    /// Path components which have been followed, held in the same order in which they were
    /// provided to the `query()` call.
    visited_names: Vec<String>,
    /// Current depth of the query.
    depth: u64,
}

impl Query {
    fn new(base_key: Key, path: &[String]) -> Self {
        Query {
            base_key,
            current_key: base_key.normalize(),
            unvisited_names: path.iter().cloned().collect(),
            visited_names: Vec::new(),
            visited_keys: HashSet::new(),
            depth: 0,
        }
    }

    /// Panics if `unvisited_names` is empty.
    fn next_name(&mut self) -> &String {
        let next_name = self.unvisited_names.pop_front().unwrap();
        self.visited_names.push(next_name);
        self.visited_names.last().unwrap()
    }

    fn navigate(&mut self, key: Key) {
        self.current_key = key.normalize();
        self.depth += 1;
    }

    fn navigate_for_named_key(&mut self, named_key: Key) {
        if let Key::NamedKey(_) = &named_key {
            self.current_key = named_key.normalize();
        }
    }

    fn into_not_found_result(self, msg_prefix: &str) -> TrackingCopyQueryResult {
        let msg = format!("{} at path: {}", msg_prefix, self.current_path());
        TrackingCopyQueryResult::ValueNotFound(msg)
    }

    fn into_circular_ref_result(self) -> TrackingCopyQueryResult {
        let msg = format!(
            "{:?} has formed a circular reference at path: {}",
            self.current_key,
            self.current_path()
        );
        TrackingCopyQueryResult::CircularReference(msg)
    }

    fn into_depth_limit_result(self) -> TrackingCopyQueryResult {
        TrackingCopyQueryResult::DepthLimit { depth: self.depth }
    }

    fn current_path(&self) -> String {
        let mut path = format!("{:?}", self.base_key);
        for name in &self.visited_names {
            path.push('/');
            path.push_str(name);
        }
        path
    }
}

/// Keeps track of already accessed keys.
/// We deliberately separate cached Reads from cached mutations
/// because we want to invalidate Reads' cache so it doesn't grow too fast.
#[derive(Clone, Debug)]
pub struct GenericTrackingCopyCache<M: Copy + Debug> {
    max_cache_size: usize,
    current_cache_size: usize,
    reads_cached: LinkedHashMap<Key, StoredValue>,
    muts_cached: BTreeMap<KeyWithByteRepr, StoredValue>,
    prunes_cached: BTreeSet<Key>,
    meter: M,
}

impl<M: Meter<Key, StoredValue> + Copy + Default> GenericTrackingCopyCache<M> {
    /// Creates instance of `TrackingCopyCache` with specified `max_cache_size`,
    /// above which least-recently-used elements of the cache are invalidated.
    /// Measurements of elements' "size" is done with the usage of `Meter`
    /// instance.
    pub fn new(max_cache_size: usize, meter: M) -> GenericTrackingCopyCache<M> {
        GenericTrackingCopyCache {
            max_cache_size,
            current_cache_size: 0,
            reads_cached: LinkedHashMap::new(),
            muts_cached: BTreeMap::new(),
            prunes_cached: BTreeSet::new(),
            meter,
        }
    }

    /// Creates instance of `TrackingCopyCache` with specified `max_cache_size`, above which
    /// least-recently-used elements of the cache are invalidated. Measurements of elements' "size"
    /// is done with the usage of default `Meter` instance.
    pub fn new_default(max_cache_size: usize) -> GenericTrackingCopyCache<M> {
        GenericTrackingCopyCache::new(max_cache_size, M::default())
    }

    /// Inserts `key` and `value` pair to Read cache.
    pub fn insert_read(&mut self, key: Key, value: StoredValue) {
        let element_size = Meter::measure(&self.meter, &key, &value);
        self.reads_cached.insert(key, value);
        self.current_cache_size += element_size;
        while self.current_cache_size > self.max_cache_size {
            match self.reads_cached.pop_front() {
                Some((k, v)) => {
                    let element_size = Meter::measure(&self.meter, &k, &v);
                    self.current_cache_size -= element_size;
                }
                None => break,
            }
        }
    }

    /// Inserts `key` and `value` pair to Write/Add cache.
    pub fn insert_write(&mut self, key: Key, value: StoredValue) {
        let kb = KeyWithByteRepr::new(key);
        self.prunes_cached.remove(&key);
        self.muts_cached.insert(kb, value);
    }

    /// Inserts `key` and `value` pair to Write/Add cache.
    pub fn insert_prune(&mut self, key: Key) {
        self.prunes_cached.insert(key);
    }

    /// Gets value from `key` in the cache.
    pub fn get(&mut self, key: &Key) -> Option<&StoredValue> {
        if self.prunes_cached.contains(key) {
            // the item is marked for pruning and therefore
            // is no longer accessible.
            return None;
        }
        let kb = KeyWithByteRepr::new(*key);
        if let Some(value) = self.muts_cached.get(&kb) {
            return Some(value);
        };

        self.reads_cached.get_refresh(key).map(|v| &*v)
    }

    /// Get cached items by prefix.
    fn get_muts_cached_by_byte_prefix(&self, prefix: &[u8]) -> Vec<Key> {
        self.muts_cached
            .range(prefix.to_vec()..)
            .take_while(|(key, _)| key.starts_with(prefix))
            .map(|(key, _)| key.to_key())
            .collect()
    }

    /// Does the prune cache contain key.
    pub fn is_pruned(&self, key: &Key) -> bool {
        self.prunes_cached.contains(key)
    }

    pub(self) fn into_muts(self) -> (BTreeMap<KeyWithByteRepr, StoredValue>, BTreeSet<Key>) {
        (self.muts_cached, self.prunes_cached)
    }
}

/// A helper type for `TrackingCopyCache` that allows convenient storage and access
/// to keys as bytes.
/// Its equality and ordering is based on the byte representation of the key.
#[derive(Debug, Clone)]
struct KeyWithByteRepr(Key, Vec<u8>);

impl KeyWithByteRepr {
    #[inline]
    fn new(key: Key) -> Self {
        let bytes = key.to_bytes().expect("should always serialize a Key");
        KeyWithByteRepr(key, bytes)
    }

    #[inline]
    fn starts_with(&self, prefix: &[u8]) -> bool {
        self.1.starts_with(prefix)
    }

    #[inline]
    fn to_key(&self) -> Key {
        self.0
    }
}

impl Borrow<Vec<u8>> for KeyWithByteRepr {
    #[inline]
    fn borrow(&self) -> &Vec<u8> {
        &self.1
    }
}

impl PartialEq for KeyWithByteRepr {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.1 == other.1
    }
}

impl Eq for KeyWithByteRepr {}

impl PartialOrd for KeyWithByteRepr {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for KeyWithByteRepr {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.1.cmp(&other.1)
    }
}

/// An alias for a `TrackingCopyCache` with `HeapSize` as the meter.
pub type TrackingCopyCache = GenericTrackingCopyCache<HeapSize>;

/// An interface for the global state that caches all operations (reads and writes) instead of
/// applying them directly to the state. This way the state remains unmodified, while the user can
/// interact with it as if it was being modified in real time.
#[derive(Clone)]
pub struct TrackingCopy<R> {
    reader: Arc<R>,
    cache: TrackingCopyCache,
    effects: Effects,
    max_query_depth: u64,
    messages: Messages,
    enable_addressable_entity: bool,
}

/// Result of executing an "add" operation on a value in the state.
#[derive(Debug)]
pub enum AddResult {
    /// The operation was successful.
    Success,
    /// The key was not found.
    KeyNotFound(Key),
    /// There was a type mismatch between the stored value and the value being added.
    TypeMismatch(StoredValueTypeMismatch),
    /// Serialization error.
    Serialization(bytesrepr::Error),
    /// Transform error.
    Transform(TransformError),
}

impl From<CLValueError> for AddResult {
    fn from(error: CLValueError) -> Self {
        match error {
            CLValueError::Serialization(error) => AddResult::Serialization(error),
            CLValueError::Type(type_mismatch) => {
                let expected = format!("{:?}", type_mismatch.expected);
                let found = format!("{:?}", type_mismatch.found);
                AddResult::TypeMismatch(StoredValueTypeMismatch::new(expected, found))
            }
        }
    }
}

/// A helper type for `TrackingCopy` that represents a key-value pair.
pub type TrackingCopyParts = (TrackingCopyCache, Effects, Messages);

impl<R: StateReader<Key, StoredValue>> TrackingCopy<R>
where
    R: StateReader<Key, StoredValue, Error = GlobalStateError>,
{
    /// Creates a new `TrackingCopy` using the `reader` as the interface to the state.
    pub fn new(
        reader: R,
        max_query_depth: u64,
        enable_addressable_entity: bool,
    ) -> TrackingCopy<R> {
        TrackingCopy {
            reader: Arc::new(reader),
            // TODO: Should `max_cache_size` be a fraction of wasm memory limit?
            cache: GenericTrackingCopyCache::new(1024 * 16, HeapSize),
            effects: Effects::new(),
            max_query_depth,
            messages: Vec::new(),
            enable_addressable_entity,
        }
    }

    /// Returns the `reader` used to access the state.
    pub fn reader(&self) -> &R {
        &self.reader
    }

    /// Returns a shared reference to the `reader` used to access the state.
    pub fn shared_reader(&self) -> Arc<R> {
        Arc::clone(&self.reader)
    }

    /// Creates a new `TrackingCopy` using the `reader` as the interface to the state.
    /// Returns a new `TrackingCopy` instance that is a snapshot of the current state, allowing
    /// further changes to be made.
    ///
    /// This method creates a new `TrackingCopy` using the current instance (including its
    /// mutations) as the base state to read against. Mutations made to the new `TrackingCopy`
    /// will not impact the original instance.
    ///
    /// Note: Currently, there is no `join` or `merge` function to bring changes from a fork back to
    /// the main `TrackingCopy`. Therefore, forking should be done repeatedly, which is
    /// suboptimal and will be improved in the future.
    pub fn fork(&self) -> TrackingCopy<&TrackingCopy<R>> {
        TrackingCopy::new(self, self.max_query_depth, self.enable_addressable_entity)
    }

    /// Returns a new `TrackingCopy` instance that is a snapshot of the current state, allowing
    /// further changes to be made.
    ///
    /// This method creates a new `TrackingCopy` using the current instance (including its
    /// mutations) as the base state to read against. Mutations made to the new `TrackingCopy`
    /// will not impact the original instance.
    ///
    /// Note: Currently, there is no `join` or `merge` function to bring changes from a fork back to
    /// the main `TrackingCopy`. This method is an alternative to the `fork` method and is
    /// provided for clarity and consistency in naming.
    pub fn fork2(&self) -> Self {
        TrackingCopy {
            reader: Arc::clone(&self.reader),
            cache: self.cache.clone(),
            effects: self.effects.clone(),
            max_query_depth: self.max_query_depth,
            messages: self.messages.clone(),
            enable_addressable_entity: self.enable_addressable_entity,
        }
    }

    /// Applies the changes to the state.
    ///
    /// This is a low-level function that should be used only by the execution engine. The purpose
    /// of this function is to apply the changes to the state from a forked tracking copy. Once
    /// caller decides that the changes are valid, they can be applied to the state and the
    /// processing can resume.
    pub fn apply_changes(
        &mut self,
        effects: Effects,
        cache: TrackingCopyCache,
        messages: Messages,
    ) {
        self.effects = effects;
        self.cache = cache;
        self.messages = messages;
    }

    /// Returns a copy of the execution effects cached by this instance.
    pub fn effects(&self) -> Effects {
        self.effects.clone()
    }

    /// Returns copy of cache.
    pub fn cache(&self) -> TrackingCopyCache {
        self.cache.clone()
    }

    /// Destructure cached entries.
    pub fn destructure(self) -> (Vec<(Key, StoredValue)>, BTreeSet<Key>, Effects) {
        let (writes, prunes) = self.cache.into_muts();
        let writes: Vec<(Key, StoredValue)> = writes.into_iter().map(|(k, v)| (k.0, v)).collect();

        (writes, prunes, self.effects)
    }

    /// Enable the addressable entity and migrate accounts/contracts to entities.
    pub fn enable_addressable_entity(&self) -> bool {
        self.enable_addressable_entity
    }

    /// Get record by key.
    pub fn get(&mut self, key: &Key) -> Result<Option<StoredValue>, TrackingCopyError> {
        if let Some(value) = self.cache.get(key) {
            return Ok(Some(value.to_owned()));
        }
        match self.reader.read(key) {
            Ok(ret) => {
                if let Some(value) = ret {
                    self.cache.insert_read(*key, value.to_owned());
                    Ok(Some(value))
                } else {
                    Ok(None)
                }
            }
            Err(err) => Err(TrackingCopyError::Storage(err)),
        }
    }

    /// Gets the set of keys in the state whose tag is `key_tag`.
    pub fn get_keys(&self, key_tag: &KeyTag) -> Result<BTreeSet<Key>, TrackingCopyError> {
        self.get_by_byte_prefix(&[*key_tag as u8])
    }

    /// Get keys by prefix.
    pub fn get_keys_by_prefix(
        &self,
        key_prefix: &KeyPrefix,
    ) -> Result<BTreeSet<Key>, TrackingCopyError> {
        let byte_prefix = key_prefix
            .to_bytes()
            .map_err(TrackingCopyError::BytesRepr)?;
        self.get_by_byte_prefix(&byte_prefix)
    }

    /// Gets the set of keys in the state by a byte prefix.
    pub(crate) fn get_by_byte_prefix(
        &self,
        byte_prefix: &[u8],
    ) -> Result<BTreeSet<Key>, TrackingCopyError> {
        let ret = self.keys_with_prefix(byte_prefix)?.into_iter().collect();
        Ok(ret)
    }

    /// Reads the value stored under `key`.
    pub fn read(&mut self, key: &Key) -> Result<Option<StoredValue>, TrackingCopyError> {
        let normalized_key = key.normalize();
        if let Some(value) = self.get(&normalized_key)? {
            self.effects
                .push(TransformV2::new(normalized_key, TransformKindV2::Identity));
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    /// Reads the first value stored under the keys in `keys`.
    pub fn read_first(&mut self, keys: &[&Key]) -> Result<Option<StoredValue>, TrackingCopyError> {
        for key in keys {
            if let Some(value) = self.read(key)? {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    /// Writes `value` under `key`. Note that the written value is only cached.
    pub fn write(&mut self, key: Key, value: StoredValue) {
        let normalized_key = key.normalize();
        self.cache.insert_write(normalized_key, value.clone());
        let transform = TransformV2::new(normalized_key, TransformKindV2::Write(value));
        self.effects.push(transform);
    }

    /// Caches the emitted message and writes the message topic summary under the specified key.
    ///
    /// This function does not check the types for the key and the value so the caller should
    /// correctly set the type. The `message_topic_key` should be of the `Key::MessageTopic`
    /// variant and the `message_topic_summary` should be of the `StoredValue::Message` variant.
    #[allow(clippy::too_many_arguments)]
    pub fn emit_message(
        &mut self,
        message_topic_key: Key,
        message_topic_summary: StoredValue,
        message_key: Key,
        message_value: StoredValue,
        block_message_count_value: StoredValue,
        message: Message,
    ) {
        self.write(message_key, message_value);
        self.write(message_topic_key, message_topic_summary);
        self.write(
            Key::BlockGlobal(BlockGlobalAddr::MessageCount),
            block_message_count_value,
        );
        self.messages.push(message);
    }

    /// Prunes a `key`.
    pub fn prune(&mut self, key: Key) {
        let normalized_key = key.normalize();
        self.cache.insert_prune(normalized_key);
        self.effects.push(TransformV2::new(
            normalized_key,
            TransformKindV2::Prune(key),
        ));
    }

    /// Ok(None) represents missing key to which we want to "add" some value.
    /// Ok(Some(unit)) represents successful operation.
    /// Err(error) is reserved for unexpected errors when accessing global
    /// state.
    pub fn add(&mut self, key: Key, value: StoredValue) -> Result<AddResult, TrackingCopyError> {
        let normalized_key = key.normalize();
        let current_value = match self.get(&normalized_key)? {
            None => return Ok(AddResult::KeyNotFound(normalized_key)),
            Some(current_value) => current_value,
        };

        let type_name = value.type_name();
        let mismatch = || {
            Ok(AddResult::TypeMismatch(StoredValueTypeMismatch::new(
                "I32, U64, U128, U256, U512 or (String, Key) tuple".to_string(),
                type_name,
            )))
        };

        let transform_kind = match value {
            StoredValue::CLValue(cl_value) => match *cl_value.cl_type() {
                CLType::I32 => match cl_value.into_t() {
                    Ok(value) => TransformKindV2::AddInt32(value),
                    Err(error) => return Ok(AddResult::from(error)),
                },
                CLType::U64 => match cl_value.into_t() {
                    Ok(value) => TransformKindV2::AddUInt64(value),
                    Err(error) => return Ok(AddResult::from(error)),
                },
                CLType::U128 => match cl_value.into_t() {
                    Ok(value) => TransformKindV2::AddUInt128(value),
                    Err(error) => return Ok(AddResult::from(error)),
                },
                CLType::U256 => match cl_value.into_t() {
                    Ok(value) => TransformKindV2::AddUInt256(value),
                    Err(error) => return Ok(AddResult::from(error)),
                },
                CLType::U512 => match cl_value.into_t() {
                    Ok(value) => TransformKindV2::AddUInt512(value),
                    Err(error) => return Ok(AddResult::from(error)),
                },
                _ => {
                    if *cl_value.cl_type() == casper_types::named_key_type() {
                        match cl_value.into_t() {
                            Ok((name, key)) => {
                                let mut named_keys = NamedKeys::new();
                                named_keys.insert(name, key);
                                TransformKindV2::AddKeys(named_keys)
                            }
                            Err(error) => return Ok(AddResult::from(error)),
                        }
                    } else {
                        return mismatch();
                    }
                }
            },
            _ => return mismatch(),
        };

        match transform_kind.clone().apply(current_value) {
            Ok(TransformInstruction::Store(new_value)) => {
                self.cache.insert_write(normalized_key, new_value);
                self.effects
                    .push(TransformV2::new(normalized_key, transform_kind));
                Ok(AddResult::Success)
            }
            Ok(TransformInstruction::Prune(key)) => {
                self.cache.insert_prune(normalized_key);
                self.effects.push(TransformV2::new(
                    normalized_key,
                    TransformKindV2::Prune(key),
                ));
                Ok(AddResult::Success)
            }
            Err(TransformError::TypeMismatch(type_mismatch)) => {
                Ok(AddResult::TypeMismatch(type_mismatch))
            }
            Err(TransformError::Serialization(error)) => Ok(AddResult::Serialization(error)),
            Err(transform_error) => Ok(AddResult::Transform(transform_error)),
        }
    }

    /// Returns a copy of the messages cached by this instance.
    pub fn messages(&self) -> Messages {
        self.messages.clone()
    }

    /// Calling `query()` avoids calling into `self.cache`, so this will not return any values
    /// written or mutated in this `TrackingCopy` via previous calls to `write()` or `add()`, since
    /// these updates are only held in `self.cache`.
    ///
    /// The intent is that `query()` is only used to satisfy `QueryRequest`s made to the server.
    /// Other EE internal use cases should call `read()` or `get()` in order to retrieve cached
    /// values.
    pub fn query(
        &self,
        base_key: Key,
        path: &[String],
    ) -> Result<TrackingCopyQueryResult, TrackingCopyError> {
        let mut query = Query::new(base_key, path);

        let mut proofs = Vec::new();

        loop {
            if query.depth >= self.max_query_depth {
                return Ok(query.into_depth_limit_result());
            }

            if !query.visited_keys.insert(query.current_key) {
                return Ok(query.into_circular_ref_result());
            }

            let stored_value = match self.reader.read_with_proof(&query.current_key)? {
                None => {
                    return Ok(query.into_not_found_result("Failed to find base key"));
                }
                Some(stored_value) => stored_value,
            };

            let value = stored_value.value().to_owned();

            // Following code does a patching on the `StoredValue` that unwraps an inner
            // `DictionaryValue` for dictionaries only.
            let value = match handle_stored_dictionary_value(query.current_key, value) {
                Ok(patched_stored_value) => patched_stored_value,
                Err(error) => {
                    return Ok(query.into_not_found_result(&format!(
                        "Failed to retrieve dictionary value: {}",
                        error
                    )));
                }
            };

            proofs.push(stored_value);

            if query.unvisited_names.is_empty() && !query.current_key.is_named_key() {
                return Ok(TrackingCopyQueryResult::Success { value, proofs });
            }

            let stored_value: &StoredValue = proofs
                .last()
                .map(|r| r.value())
                .expect("but we just pushed");

            match stored_value {
                StoredValue::Account(account) => {
                    let name = query.next_name();
                    if let Some(key) = account.named_keys().get(name) {
                        query.navigate(*key);
                    } else {
                        let msg_prefix = format!("Name {} not found in Account", name);
                        return Ok(query.into_not_found_result(&msg_prefix));
                    }
                }
                StoredValue::Contract(contract) => {
                    let name = query.next_name();
                    if let Some(key) = contract.named_keys().get(name) {
                        query.navigate(*key);
                    } else {
                        let msg_prefix = format!("Name {} not found in Contract", name);
                        return Ok(query.into_not_found_result(&msg_prefix));
                    }
                }
                StoredValue::NamedKey(named_key_value) => {
                    match query.visited_names.last() {
                        Some(expected_name) => match named_key_value.get_name() {
                            Ok(actual_name) => {
                                if &actual_name != expected_name {
                                    return Ok(query.into_not_found_result(
                                        "Queried and retrieved names do not match",
                                    ));
                                } else if let Ok(key) = named_key_value.get_key() {
                                    query.navigate(key)
                                } else {
                                    return Ok(query
                                        .into_not_found_result("Failed to parse CLValue as Key"));
                                }
                            }
                            Err(_) => {
                                return Ok(query
                                    .into_not_found_result("Failed to parse CLValue as String"));
                            }
                        },
                        None if path.is_empty() => {
                            return Ok(TrackingCopyQueryResult::Success { value, proofs });
                        }
                        None => return Ok(query.into_not_found_result("No visited names")),
                    }
                }
                StoredValue::CLValue(cl_value) if cl_value.cl_type() == &CLType::Key => {
                    if let Ok(key) = cl_value.to_owned().into_t::<Key>() {
                        query.navigate(key);
                    } else {
                        return Ok(query.into_not_found_result("Failed to parse CLValue as Key"));
                    }
                }
                StoredValue::CLValue(cl_value) => {
                    let msg_prefix = format!(
                        "Query cannot continue as {:?} is not an account, contract nor key to \
                        such.  Value found",
                        cl_value
                    );
                    return Ok(query.into_not_found_result(&msg_prefix));
                }
                StoredValue::AddressableEntity(_) => {
                    let current_key = query.current_key;
                    let name = query.next_name();

                    if let Key::AddressableEntity(addr) = current_key {
                        let named_key_addr = match NamedKeyAddr::new_from_string(addr, name.clone())
                        {
                            Ok(named_key_addr) => Key::NamedKey(named_key_addr),
                            Err(error) => {
                                let msg_prefix = format!("{}", error);
                                return Ok(query.into_not_found_result(&msg_prefix));
                            }
                        };
                        query.navigate_for_named_key(named_key_addr);
                    } else {
                        let msg_prefix = "Invalid base key".to_string();
                        return Ok(query.into_not_found_result(&msg_prefix));
                    }
                }
                StoredValue::ContractWasm(_) => {
                    return Ok(query.into_not_found_result("ContractWasm value found."));
                }
                StoredValue::ContractPackage(_) => {
                    return Ok(query.into_not_found_result("ContractPackage value found."));
                }
                StoredValue::SmartContract(_) => {
                    return Ok(query.into_not_found_result("Package value found."));
                }
                StoredValue::ByteCode(_) => {
                    return Ok(query.into_not_found_result("ByteCode value found."));
                }
                StoredValue::Transfer(_) => {
                    return Ok(query.into_not_found_result("Legacy Transfer value found."));
                }
                StoredValue::DeployInfo(_) => {
                    return Ok(query.into_not_found_result("DeployInfo value found."));
                }
                StoredValue::EraInfo(_) => {
                    return Ok(query.into_not_found_result("EraInfo value found."));
                }
                StoredValue::Bid(_) => {
                    return Ok(query.into_not_found_result("Bid value found."));
                }
                StoredValue::BidKind(_) => {
                    return Ok(query.into_not_found_result("BidKind value found."));
                }
                StoredValue::Withdraw(_) => {
                    return Ok(query.into_not_found_result("WithdrawPurses value found."));
                }
                StoredValue::Unbonding(_) => {
                    return Ok(query.into_not_found_result("Unbonding value found."));
                }
                StoredValue::MessageTopic(_) => {
                    return Ok(query.into_not_found_result("MessageTopic value found."));
                }
                StoredValue::Message(_) => {
                    return Ok(query.into_not_found_result("Message value found."));
                }
                StoredValue::EntryPoint(_) => {
                    return Ok(query.into_not_found_result("EntryPoint value found."));
                }
                StoredValue::Prepayment(_) => {
                    return Ok(query.into_not_found_result("Prepayment value found."))
                }
                StoredValue::RawBytes(_) => {
                    return Ok(query.into_not_found_result("RawBytes value found."));
                }
            }
        }
    }
}

/// The purpose of this implementation is to allow a "snapshot" mechanism for
/// TrackingCopy. The state of a TrackingCopy (including the effects of
/// any transforms it has accumulated) can be read using an immutable
/// reference to that TrackingCopy via this trait implementation. See
/// `TrackingCopy::fork` for more information.
impl<R: StateReader<Key, StoredValue>> StateReader<Key, StoredValue> for &TrackingCopy<R> {
    type Error = R::Error;

    fn read(&self, key: &Key) -> Result<Option<StoredValue>, Self::Error> {
        let kb = KeyWithByteRepr::new(*key);
        if let Some(value) = self.cache.muts_cached.get(&kb) {
            return Ok(Some(value.to_owned()));
        }
        if let Some(value) = self.reader.read(key)? {
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    fn read_with_proof(
        &self,
        key: &Key,
    ) -> Result<Option<TrieMerkleProof<Key, StoredValue>>, Self::Error> {
        self.reader.read_with_proof(key)
    }

    fn keys_with_prefix(&self, byte_prefix: &[u8]) -> Result<Vec<Key>, Self::Error> {
        let keys = self.reader.keys_with_prefix(byte_prefix)?;

        let ret = keys
            .into_iter()
            // don't include keys marked for pruning
            .filter(|key| !self.cache.is_pruned(key))
            // there may be newly inserted keys which have not been committed yet
            .chain(self.cache.get_muts_cached_by_byte_prefix(byte_prefix))
            .collect();
        Ok(ret)
    }
}

/// Error conditions of a proof validation.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum ValidationError {
    /// The path should not have a different length than the proof less one.
    #[error("The path should not have a different length than the proof less one.")]
    PathLengthDifferentThanProofLessOne,

    /// The provided key does not match the key in the proof.
    #[error("The provided key does not match the key in the proof.")]
    UnexpectedKey,

    /// The provided value does not match the value in the proof.
    #[error("The provided value does not match the value in the proof.")]
    UnexpectedValue,

    /// The proof hash is invalid.
    #[error("The proof hash is invalid.")]
    InvalidProofHash,

    /// The path went cold.
    #[error("The path went cold.")]
    PathCold,

    /// (De)serialization error.
    #[error("Serialization error: {0}")]
    BytesRepr(bytesrepr::Error),

    /// Key is not a URef.
    #[error("Key is not a URef")]
    KeyIsNotAURef(Key),

    /// Error converting a stored value to a [`Key`].
    #[error("Failed to convert stored value to key")]
    ValueToCLValueConversion,

    /// CLValue conversion error.
    #[error("{0}")]
    CLValueError(CLValueError),
}

impl From<CLValueError> for ValidationError {
    fn from(err: CLValueError) -> Self {
        ValidationError::CLValueError(err)
    }
}

impl From<bytesrepr::Error> for ValidationError {
    fn from(error: bytesrepr::Error) -> Self {
        Self::BytesRepr(error)
    }
}

/// Validates proof of the query.
///
/// Returns [`ValidationError`] for any of
pub fn validate_query_proof(
    hash: &Digest,
    proofs: &[TrieMerkleProof<Key, StoredValue>],
    expected_first_key: &Key,
    path: &[String],
    expected_value: &StoredValue,
) -> Result<(), ValidationError> {
    if proofs.len() != path.len() + 1 {
        return Err(ValidationError::PathLengthDifferentThanProofLessOne);
    }

    let mut proofs_iter = proofs.iter();
    let mut path_components_iter = path.iter();

    // length check above means we are safe to unwrap here
    let first_proof = proofs_iter.next().unwrap();

    if first_proof.key() != &expected_first_key.normalize() {
        return Err(ValidationError::UnexpectedKey);
    }

    if hash != &compute_state_hash(first_proof)? {
        return Err(ValidationError::InvalidProofHash);
    }

    let mut proof_value = first_proof.value();

    for proof in proofs_iter {
        let named_keys = match proof_value {
            StoredValue::Account(account) => account.named_keys(),
            StoredValue::Contract(contract) => contract.named_keys(),
            _ => return Err(ValidationError::PathCold),
        };

        let path_component = match path_components_iter.next() {
            Some(path_component) => path_component,
            None => return Err(ValidationError::PathCold),
        };

        let key = match named_keys.get(path_component) {
            Some(key) => key,
            None => return Err(ValidationError::PathCold),
        };

        if proof.key() != &key.normalize() {
            return Err(ValidationError::UnexpectedKey);
        }

        if hash != &compute_state_hash(proof)? {
            return Err(ValidationError::InvalidProofHash);
        }

        proof_value = proof.value();
    }

    if proof_value != expected_value {
        return Err(ValidationError::UnexpectedValue);
    }

    Ok(())
}

/// Validates proof of the query.
///
/// Returns [`ValidationError`] for any of
pub fn validate_query_merkle_proof(
    hash: &Digest,
    proofs: &[TrieMerkleProof<Key, StoredValue>],
    expected_key_trace: &[Key],
    expected_value: &StoredValue,
) -> Result<(), ValidationError> {
    let expected_len = expected_key_trace.len();
    if proofs.len() != expected_len {
        return Err(ValidationError::PathLengthDifferentThanProofLessOne);
    }

    let proof_keys: Vec<Key> = proofs.iter().map(|proof| *proof.key()).collect();

    if !expected_key_trace.eq(&proof_keys) {
        return Err(ValidationError::UnexpectedKey);
    }

    if expected_value != proofs[expected_len - 1].value() {
        return Err(ValidationError::UnexpectedValue);
    }

    let mut proofs_iter = proofs.iter();

    // length check above means we are safe to unwrap here
    let first_proof = proofs_iter.next().unwrap();

    if hash != &compute_state_hash(first_proof)? {
        return Err(ValidationError::InvalidProofHash);
    }

    Ok(())
}

/// Validates a proof of a balance request.
pub fn validate_balance_proof(
    hash: &Digest,
    balance_proof: &TrieMerkleProof<Key, StoredValue>,
    expected_purse_key: Key,
    expected_motes: &U512,
) -> Result<(), ValidationError> {
    let expected_balance_key = expected_purse_key
        .into_uref()
        .map(|uref| Key::Balance(uref.addr()))
        .ok_or_else(|| ValidationError::KeyIsNotAURef(expected_purse_key.to_owned()))?;

    if balance_proof.key() != &expected_balance_key.normalize() {
        return Err(ValidationError::UnexpectedKey);
    }

    if hash != &compute_state_hash(balance_proof)? {
        return Err(ValidationError::InvalidProofHash);
    }

    let balance_proof_stored_value = balance_proof.value().to_owned();

    let balance_proof_clvalue: CLValue = balance_proof_stored_value
        .try_into()
        .map_err(|_| ValidationError::ValueToCLValueConversion)?;

    let balance_motes: U512 = balance_proof_clvalue.into_t()?;

    if expected_motes != &balance_motes {
        return Err(ValidationError::UnexpectedValue);
    }

    Ok(())
}

use crate::global_state::{
    error::Error,
    state::{
        lmdb::{make_temporary_global_state, LmdbGlobalStateView},
        StateProvider,
    },
};
use tempfile::TempDir;

/// Creates a temp global state with initial state and checks out a tracking copy on it.
pub fn new_temporary_tracking_copy(
    initial_data: impl IntoIterator<Item = (Key, StoredValue)>,
    max_query_depth: Option<u64>,
    enable_addressable_entity: bool,
) -> (TrackingCopy<LmdbGlobalStateView>, TempDir) {
    let (global_state, state_root_hash, tempdir) = make_temporary_global_state(initial_data);

    let reader = global_state
        .checkout(state_root_hash)
        .expect("Checkout should not throw errors.")
        .expect("Root hash should exist.");

    let query_depth = max_query_depth.unwrap_or(DEFAULT_MAX_QUERY_DEPTH);

    (
        TrackingCopy::new(reader, query_depth, enable_addressable_entity),
        tempdir,
    )
}
