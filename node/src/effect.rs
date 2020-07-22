//! Effects subsystem.
//!
//! Effects describe things that the creator of the effect intends to happen, producing a value upon
//! completion. They are, in fact, futures.
//!
//! A pinned, boxed future returning an event is called an effect and typed as an `Effect<Ev>`,
//! where `Ev` is the event's type. Generally, `Ev` is an Event enum defined at the top level of
//! each component in the `crate::components` module.
//!
//! ## Using effects
//!
//! To create an effect, an `EffectBuilder` will be passed in from the relevant reactor. For
//! example, given an effect builder `effect_builder`, we can create a `set_timeout` future and turn
//! it into an effect:
//!
//! ```ignore
//! use std::time::Duration;
//! use casperlabs_node::effect::EffectExt;
//!
//! enum Event {
//!     ThreeSecondsElapsed(Duration)
//! }
//!
//! effect_builder
//!     .set_timeout(Duration::from_secs(3))
//!     .event(Event::ThreeSecondsElapsed);
//! ```
//!
//! This example will produce an effect that, after three seconds, creates an
//! `Event::ThreeSecondsElapsed`. Note that effects do nothing on their own, they need to be passed
//! to a [`reactor`](../reactor/index.html) to be executed.
//!
//! ## Arbitrary effects
//!
//! While it is technically possible to turn any future into an effect, it is advisable to only use
//! the effects explicitly listed in this module through traits to create them. Post-processing on
//! effects to turn them into events should also be kept brief.
//!
//! ## Announcements and effects
//!
//! Some effects can be further classified into either announcements or requests, although these
//! properties are not reflected in the type system.
//!
//! **Announcements** are events emitted by components that are essentially "fire-and-forget"; the
//! component will never expect an answer for these and does not rely on them being handled. It is
//! also conceivable that they are being cloned and dispatched to multiple components by the
//! reactor.
//!
//! A good example is the arrival of a new deploy passed in by a client. Depending on the setup it
//! may be stored, buffered or, in certain testing setups, just discarded. None of this is a concern
//! of the component that talks to the client and deserializes the incoming deploy though, which
//! considers the deploy no longer its concern after it has returned an announcement effect.
//!
//! **Requests** are complex effects that are used when a component needs something from
//! outside of itself (typically to be provided by another component); a request requires an
//! eventual response.
//!
//! A request **must** have a `Responder` field, which a handler of a request **must** call at
//! some point. Failing to do so will result in a resource leak.

pub mod announcements;
pub mod requests;

use std::{
    any::type_name,
    collections::HashSet,
    fmt::{self, Debug, Display, Formatter},
    future::Future,
    time::{Duration, Instant},
};

use futures::{channel::oneshot, future::BoxFuture, FutureExt};
use semver::Version;
use smallvec::{smallvec, SmallVec};
use tracing::error;

use crate::{
    components::{
        consensus::BlockContext,
        contract_runtime::core::engine_state::{self, genesis::GenesisResult},
        deploy_buffer::BlockLimits,
        storage::{self, DeployHashes, DeployHeaderResults, DeployResults, StorageType, Value},
    },
    reactor::{EventQueueHandle, QueueKind},
    types::{Deploy, DeployHash, ExecutedBlock, ProtoBlock},
    Chainspec,
};
use announcements::{
    ApiServerAnnouncement, ConsensusAnnouncement, NetworkAnnouncement, StorageAnnouncement,
};
use requests::{
    ContractRuntimeRequest, DeployFetcherRequest, DeployQueueRequest, MetricsRequest,
    NetworkRequest, StorageRequest,
};

/// A pinned, boxed future that produces one or more events.
pub type Effect<Ev> = BoxFuture<'static, Multiple<Ev>>;

/// Multiple effects in a container.
pub type Effects<Ev> = Multiple<Effect<Ev>>;

/// A small collection of rarely more than two items.
///
/// Stored in a `SmallVec` to avoid allocations in case there are less than three items grouped. The
/// size of two items is chosen because one item is the most common use case, and large items are
/// typically boxed. In the latter case two pointers and one enum variant discriminator is almost
/// the same size as an empty vec, which is two pointers.
type Multiple<T> = SmallVec<[T; 2]>;

/// A responder satisfying a request.
pub struct Responder<T>(oneshot::Sender<T>);

impl<T: 'static + Send> Responder<T> {
    fn new(sender: oneshot::Sender<T>) -> Self {
        Responder(sender)
    }
}

impl<T> Responder<T> {
    /// Send `data` to the origin of the request.
    pub async fn respond(self, data: T) {
        if self.0.send(data).is_err() {
            error!("could not send response to request down oneshot channel");
        }
    }
}

impl<T> Debug for Responder<T> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "Responder<{}>", type_name::<T>(),)
    }
}

impl<T> Display for Responder<T> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "responder({})", type_name::<T>(),)
    }
}

/// Effect extension for futures, used to convert futures into actual effects.
pub trait EffectExt: Future + Send {
    /// Finalizes a future into an effect that returns an event.
    ///
    /// The function `f` is used to translate the returned value from an effect into an event.
    fn event<U, F>(self, f: F) -> Effects<U>
    where
        F: FnOnce(Self::Output) -> U + 'static + Send,
        U: 'static,
        Self: Sized;

    /// Finalizes a future into an effect that runs but drops the result.
    fn ignore<Ev>(self) -> Effects<Ev>;
}

/// Effect extension for futures, used to convert futures returning a `Result` into two different
/// effects.
pub trait EffectResultExt {
    /// The type the future will return if `Ok`.
    type Value;
    /// The type the future will return if `Err`.
    type Error;

    /// Finalizes a future returning a `Result` into two different effects.
    ///
    /// The function `f_ok` is used to translate the returned value from an effect into an event,
    /// while the function `f_err` does the same for a potential error.
    fn result<U, F, G>(self, f_ok: F, f_err: G) -> Effects<U>
    where
        F: FnOnce(Self::Value) -> U + 'static + Send,
        G: FnOnce(Self::Error) -> U + 'static + Send,
        U: 'static;
}

/// Effect extension for futures, used to convert futures returning an `Option` into two different
/// effects.
pub trait EffectOptionExt {
    /// The type the future will return if `Some`.
    type Value;

    /// Finalizes a future returning an `Option` into two different effects.
    ///
    /// The function `f_some` is used to translate the returned value from an effect into an event,
    /// while the function `f_none` does the same for a returned `None`.
    fn option<U, F, G>(self, f_some: F, f_none: G) -> Effects<U>
    where
        F: FnOnce(Self::Value) -> U + 'static + Send,
        G: FnOnce() -> U + 'static + Send,
        U: 'static;
}

impl<T: ?Sized> EffectExt for T
where
    T: Future + Send + 'static + Sized,
{
    fn event<U, F>(self, f: F) -> Effects<U>
    where
        F: FnOnce(Self::Output) -> U + 'static + Send,
        U: 'static,
    {
        smallvec![self.map(f).map(|item| smallvec![item]).boxed()]
    }

    fn ignore<Ev>(self) -> Effects<Ev> {
        smallvec![self.map(|_| Multiple::new()).boxed()]
    }
}

impl<T, V, E> EffectResultExt for T
where
    T: Future<Output = Result<V, E>> + Send + 'static + Sized,
    T: ?Sized,
{
    type Value = V;
    type Error = E;

    fn result<U, F, G>(self, f_ok: F, f_err: G) -> Effects<U>
    where
        F: FnOnce(V) -> U + 'static + Send,
        G: FnOnce(E) -> U + 'static + Send,
        U: 'static,
    {
        smallvec![self
            .map(|result| result.map_or_else(f_err, f_ok))
            .map(|item| smallvec![item])
            .boxed()]
    }
}

impl<T, V> EffectOptionExt for T
where
    T: Future<Output = Option<V>> + Send + 'static + Sized,
    T: ?Sized,
{
    type Value = V;

    fn option<U, F, G>(self, f_some: F, f_none: G) -> Effects<U>
    where
        F: FnOnce(V) -> U + 'static + Send,
        G: FnOnce() -> U + 'static + Send,
        U: 'static,
    {
        smallvec![self
            .map(|option| option.map_or_else(f_none, f_some))
            .map(|item| smallvec![item])
            .boxed()]
    }
}

/// A builder for [`Effect`](type.Effect.html)s.
///
/// Provides methods allowing the creation of effects which need scheduled on the reactor's event
/// queue, without giving direct access to this queue.
#[derive(Debug)]
pub struct EffectBuilder<REv: 'static>(EventQueueHandle<REv>);

// Implement `Clone` and `Copy` manually, as `derive` will make it depend on `REv` otherwise.
impl<REv> Clone for EffectBuilder<REv> {
    fn clone(&self) -> Self {
        EffectBuilder(self.0)
    }
}

impl<REv> Copy for EffectBuilder<REv> {}

impl<REv> EffectBuilder<REv> {
    /// Creates a new effect builder.
    pub fn new(event_queue_handle: EventQueueHandle<REv>) -> Self {
        EffectBuilder(event_queue_handle)
    }

    /// Performs a request.
    ///
    /// Given a request `Q`, that when completed will yield a result of `T`, produces a future
    /// that will
    ///
    /// 1. create an event to send the request to the respective component (thus `Q: Into<REv>`),
    /// 2. waits for a response and returns it.
    ///
    /// This function is usually only used internally by effects implement on the effects builder,
    /// but IO components may also make use of it.
    pub(crate) async fn make_request<T, Q, F>(self, f: F, queue_kind: QueueKind) -> T
    where
        T: Send + 'static,
        Q: Into<REv>,
        F: FnOnce(Responder<T>) -> Q,
    {
        // Prepare a channel.
        let (sender, receiver) = oneshot::channel();

        // Create response function.
        let responder = Responder::new(sender);

        // Now inject the request event into the event loop.
        let request_event = f(responder).into();
        self.0.schedule(request_event, queue_kind).await;

        receiver.await.unwrap_or_else(|err| {
            // The channel should never be closed, ever.
            error!(%err, "request oneshot closed, this should not happen");
            unreachable!()
        })
    }

    /// Run and end effect immediately.
    ///
    /// Can be used to trigger events from effects when combined with `.event`. Do not use this do
    /// "do nothing", as it will still cause a task to be spawned.
    #[inline(always)]
    pub async fn immediately(self) {}

    /// Sets a timeout.
    pub(crate) async fn set_timeout(self, timeout: Duration) -> Duration {
        let then = Instant::now();
        tokio::time::delay_for(timeout).await;
        Instant::now() - then
    }

    /// Retrieve a snapshot of the nodes current metrics formatted as string.
    ///
    /// If an error occurred producing the metrics, `None` is returned.
    pub(crate) async fn get_metrics(self) -> Option<String>
    where
        REv: From<MetricsRequest>,
    {
        self.make_request(
            |responder| MetricsRequest::RenderNodeMetricsText { responder },
            QueueKind::Api,
        )
        .await
    }

    /// Sends a network message.
    ///
    /// The message is queued in "fire-and-forget" fashion, there is no guarantee that the peer
    /// will receive it.
    pub(crate) async fn send_message<I, P>(self, dest: I, payload: P)
    where
        REv: From<NetworkRequest<I, P>>,
    {
        self.make_request(
            |responder| NetworkRequest::SendMessage {
                dest,
                payload,
                responder,
            },
            QueueKind::Network,
        )
        .await
    }

    /// Broadcasts a network message.
    ///
    /// Broadcasts a network message to all peers connected at the time the message is sent.
    pub async fn broadcast_message<I, P>(self, payload: P)
    where
        REv: From<NetworkRequest<I, P>>,
    {
        self.make_request(
            |responder| NetworkRequest::Broadcast { payload, responder },
            QueueKind::Network,
        )
        .await
    }

    /// Gossips a network message.
    ///
    /// A low-level "gossip" function, selects `count` randomly chosen nodes on the network,
    /// excluding the indicated ones, and sends each a copy of the message.
    ///
    /// Returns the IDs of the chosen nodes.
    pub async fn gossip_message<I, P>(
        self,
        payload: P,
        count: usize,
        exclude: HashSet<I>,
    ) -> HashSet<I>
    where
        REv: From<NetworkRequest<I, P>>,
        I: Send + 'static,
    {
        self.make_request(
            |responder| NetworkRequest::Gossip {
                payload,
                count,
                exclude,
                responder,
            },
            QueueKind::Network,
        )
        .await
    }

    /// Announces that a network message has been received.
    pub(crate) async fn announce_message_received<I, P>(self, sender: I, payload: P)
    where
        REv: From<NetworkAnnouncement<I, P>>,
    {
        self.0
            .schedule(
                NetworkAnnouncement::MessageReceived { sender, payload },
                QueueKind::NetworkIncoming,
            )
            .await;
    }

    /// Announce that the HTTP API server has received a deploy.
    pub(crate) async fn announce_deploy_received(self, deploy: Box<Deploy>)
    where
        REv: From<ApiServerAnnouncement>,
    {
        self.0
            .schedule(
                ApiServerAnnouncement::DeployReceived { deploy },
                QueueKind::Api,
            )
            .await;
    }

    /// Announces that a (not necessarily new) deploy has been added to the store.
    pub(crate) fn announce_deploy_stored<S>(self, deploy: &S::Deploy) -> impl Future<Output = ()>
    where
        S: StorageType,
        REv: From<StorageAnnouncement<S>>,
    {
        let deploy_hash = *deploy.id();
        let deploy_header = deploy.header().clone();

        self.0.schedule(
            StorageAnnouncement::StoredDeploy {
                deploy_hash,
                deploy_header,
            },
            QueueKind::Regular,
        )
    }

    /// Puts the given block into the linear block store.
    // TODO: remove once method is used.
    #[allow(dead_code)]
    pub(crate) async fn put_block_to_storage<S>(self, block: S::Block) -> storage::Result<bool>
    where
        S: StorageType + 'static,
        REv: From<StorageRequest<S>>,
    {
        self.make_request(
            |responder| StorageRequest::PutBlock {
                block: Box::new(block),
                responder,
            },
            QueueKind::Regular,
        )
        .await
    }

    /// Gets the requested block from the linear block store.
    // TODO: remove once method is used.
    #[allow(dead_code)]
    pub(crate) async fn get_block_from_storage<S>(
        self,
        block_hash: <S::Block as Value>::Id,
    ) -> storage::Result<S::Block>
    where
        S: StorageType + 'static,
        REv: From<StorageRequest<S>>,
    {
        self.make_request(
            |responder| StorageRequest::GetBlock {
                block_hash,
                responder,
            },
            QueueKind::Regular,
        )
        .await
    }

    /// Gets the requested deploy using the `DeployFetcher`.
    // TODO: remove once method is used.
    #[allow(dead_code)]
    pub(crate) async fn fetch_deploy<I>(
        self,
        deploy_hash: DeployHash,
        peer: I,
    ) -> Option<Box<Deploy>>
    where
        REv: From<DeployFetcherRequest<I>>,
        I: Send + 'static,
    {
        self.make_request(
            |responder| DeployFetcherRequest::FetchDeploy {
                hash: deploy_hash,
                peer,
                responder,
            },
            QueueKind::Regular,
        )
        .await
    }

    /// Gets the requested block header from the linear block store.
    // TODO: remove once method is used.
    #[allow(dead_code)]
    pub(crate) async fn get_block_header_from_storage<S>(
        self,
        block_hash: <S::Block as Value>::Id,
    ) -> storage::Result<<S::Block as Value>::Header>
    where
        S: StorageType + 'static,
        REv: From<StorageRequest<S>>,
    {
        self.make_request(
            |responder| StorageRequest::GetBlockHeader {
                block_hash,
                responder,
            },
            QueueKind::Regular,
        )
        .await
    }

    /// Puts the given deploy into the deploy store.
    pub(crate) async fn put_deploy_to_storage<S>(self, deploy: S::Deploy) -> storage::Result<bool>
    where
        S: StorageType + 'static,
        REv: From<StorageRequest<S>>,
    {
        self.make_request(
            |responder| StorageRequest::PutDeploy {
                deploy: Box::new(deploy),
                responder,
            },
            QueueKind::Regular,
        )
        .await
    }

    /// Gets the requested deploys from the deploy store.
    pub(crate) async fn get_deploys_from_storage<S>(
        self,
        deploy_hashes: DeployHashes<S>,
    ) -> DeployResults<S>
    where
        S: StorageType + 'static,
        REv: From<StorageRequest<S>>,
    {
        self.make_request(
            |responder| StorageRequest::GetDeploys {
                deploy_hashes,
                responder,
            },
            QueueKind::Regular,
        )
        .await
    }

    /// Gets the requested deploy headers from the deploy store.
    // TODO: remove once method is used.
    #[allow(dead_code)]
    pub(crate) async fn get_deploy_headers_from_storage<S>(
        self,
        deploy_hashes: DeployHashes<S>,
    ) -> DeployHeaderResults<S>
    where
        S: StorageType + 'static,
        REv: From<StorageRequest<S>>,
    {
        self.make_request(
            |responder| StorageRequest::GetDeployHeaders {
                deploy_hashes,
                responder,
            },
            QueueKind::Regular,
        )
        .await
    }

    /// Lists all deploy hashes held in the deploy store.
    pub(crate) async fn list_deploys<S>(self) -> storage::Result<Vec<<S::Deploy as Value>::Id>>
    where
        S: StorageType + 'static,
        REv: From<StorageRequest<S>>,
    {
        self.make_request(
            |responder| StorageRequest::ListDeploys { responder },
            QueueKind::Regular,
        )
        .await
    }

    /// Passes the timestamp of a future block for which deploys are to be proposed.
    // TODO: Add an argument (`BlockContext`?) that contains all information necessary to select
    // deploys, e.g. the ancestors' deploys.
    pub(crate) async fn request_proto_block(
        self,
        block_context: BlockContext, /* TODO: This `BlockContext` will probably be a different
                                      * type than the context in the return value in the future */
    ) -> (ProtoBlock, BlockContext)
    where
        REv: From<DeployQueueRequest>,
    {
        // TODO: The `EffectBuilder` shouldn't contain that much logic. Move to deploy buffer.
        let limits = BlockLimits {
            size_bytes: u64::MAX,
            gas: u64::MAX,
            deploy_count: 3, // TODO
        };
        let deploys = self
            .make_request(
                |responder| DeployQueueRequest::RequestForInclusion {
                    current_instant: block_context.timestamp().millis(),
                    max_ttl: u32::MAX,
                    limits,
                    max_dependencies: u8::MAX,
                    past: Default::default(), // TODO
                    responder,
                },
                QueueKind::Regular,
            )
            .await
            .into_iter()
            .collect();
        (
            ProtoBlock {
                deploys,
                random_bit: false, // TODO
            },
            block_context,
        )
    }

    /// Passes a finalized proto-block to the contract runtime for execution.
    pub(crate) async fn execute_block(self, _proto_block: ProtoBlock) -> ExecutedBlock {
        // TODO: actually execute the block and return the relevant stuff
        todo!()
    }

    /// Checks whether the deploys included in the proto-block exist on the network.
    pub(crate) async fn validate_proto_block<I>(
        self,
        _sender: I,
        proto_block: ProtoBlock,
    ) -> (bool, ProtoBlock) {
        // TODO: check with the deploy fetcher or something whether the deploys whose hashes are
        // contained in the proto-block actually exist
        (true, proto_block)
    }

    /// Announces that a proto block has been proposed and will either be finalized or orphaned
    /// soon.
    pub(crate) async fn announce_proposed_proto_block(self, proto_block: ProtoBlock)
    where
        REv: From<ConsensusAnnouncement>,
    {
        self.0
            .schedule(
                ConsensusAnnouncement::Proposed(proto_block),
                QueueKind::Regular,
            )
            .await
    }

    /// Announces that a proto block has been finalized.
    pub(crate) async fn announce_finalized_proto_block(self, proto_block: ProtoBlock)
    where
        REv: From<ConsensusAnnouncement>,
    {
        self.0
            .schedule(
                ConsensusAnnouncement::Finalized(proto_block),
                QueueKind::Regular,
            )
            .await
    }

    /// Announces that a proto block has been orphaned.
    #[allow(dead_code)] // TODO: Detect orphaned blocks.
    pub(crate) async fn announce_orphaned_proto_block(self, proto_block: ProtoBlock)
    where
        REv: From<ConsensusAnnouncement>,
    {
        self.0
            .schedule(
                ConsensusAnnouncement::Orphaned(proto_block),
                QueueKind::Regular,
            )
            .await
    }

    /// Runs the genesis process on the contract runtime.
    pub(crate) async fn commit_genesis(
        self,
        chainspec: Chainspec,
    ) -> Result<GenesisResult, engine_state::Error>
    where
        REv: From<ContractRuntimeRequest>,
    {
        self.make_request(
            |responder| ContractRuntimeRequest::CommitGenesis {
                chainspec: Box::new(chainspec),
                responder,
            },
            QueueKind::Regular,
        )
        .await
    }

    /// Puts the given chainspec into the chainspec store.
    pub(crate) async fn put_chainspec<S>(self, chainspec: Chainspec) -> storage::Result<()>
    where
        S: StorageType + 'static,
        REv: From<StorageRequest<S>>,
    {
        self.make_request(
            |responder| StorageRequest::PutChainspec {
                chainspec: Box::new(chainspec),
                responder,
            },
            QueueKind::Regular,
        )
        .await
    }

    /// Gets the requested chainspec from the chainspec store.
    // TODO: remove once method is used.
    #[allow(dead_code)]
    pub(crate) async fn get_chainspec<S>(self, version: Version) -> storage::Result<Chainspec>
    where
        S: StorageType + 'static,
        REv: From<StorageRequest<S>>,
    {
        self.make_request(
            |responder| StorageRequest::GetChainspec { version, responder },
            QueueKind::Regular,
        )
        .await
    }
}
