use std::fmt::{self, Debug, Display, Formatter};

use serde::Serialize;
use tracing::error;

use casper_types::Transaction;

use super::{FetchItem, FetchResponder, FetchResponse};
use crate::{
    effect::{announcements::TransactionAcceptorAnnouncement, requests::FetcherRequest},
    types::NodeId,
    utils::Source,
};

/// `Fetcher` events.
#[derive(Debug, Serialize)]
pub(crate) enum Event<T: FetchItem> {
    /// The initiating event to fetch an item by its id.
    Fetch(FetcherRequest<T>),
    /// The result of the `Fetcher` getting a item from the storage component.  If the
    /// result is `None`, the item should be requested from the peer.
    GetLocallyResult {
        id: T::Id,
        peer: NodeId,
        validation_metadata: Box<T::ValidationMetadata>,
        maybe_item: Option<Box<T>>,
        responder: FetchResponder<T>,
    },
    /// An announcement from a different component that we have accepted and stored the given item.
    GotRemotely { item: Box<T>, source: Source },
    /// The result of putting the item to storage.
    PutToStorage { item: Box<T>, peer: NodeId },
    /// A different component rejected an item.
    GotInvalidRemotely { id: T::Id, source: Source },
    /// An item was not available on the remote peer.
    AbsentRemotely { id: T::Id, peer: NodeId },
    /// An item was available on the remote peer, but it chose to not provide it.
    RejectedRemotely { id: T::Id, peer: NodeId },
    /// The timeout has elapsed and we should clean up state.
    TimeoutPeer { id: T::Id, peer: NodeId },
}

impl<T: FetchItem> Event<T> {
    pub(crate) fn from_get_response_serialized_item(
        peer: NodeId,
        serialized_item: &[u8],
    ) -> Option<Self> {
        match bincode::deserialize::<FetchResponse<T, T::Id>>(serialized_item) {
            Ok(FetchResponse::Fetched(item)) => Some(Event::GotRemotely {
                item: Box::new(item),
                source: Source::Peer(peer),
            }),
            Ok(FetchResponse::NotFound(id)) => Some(Event::AbsentRemotely { id, peer }),
            Ok(FetchResponse::NotProvided(id)) => Some(Event::RejectedRemotely { id, peer }),
            Err(error) => {
                error!("failed to decode {:?} from {}: {:?}", T::TAG, peer, error);
                None
            }
        }
    }
}

impl<T: FetchItem> From<FetcherRequest<T>> for Event<T> {
    fn from(fetcher_request: FetcherRequest<T>) -> Self {
        Event::Fetch(fetcher_request)
    }
}

// A transaction fetcher knows how to update its state if transactions are coming in via the
// transaction acceptor.
impl From<TransactionAcceptorAnnouncement> for Event<Transaction> {
    fn from(announcement: TransactionAcceptorAnnouncement) -> Self {
        match announcement {
            TransactionAcceptorAnnouncement::AcceptedNewTransaction {
                transaction,
                source,
            } => Event::GotRemotely {
                item: Box::new((*transaction).clone()),
                source,
            },
            TransactionAcceptorAnnouncement::InvalidTransaction {
                transaction,
                source,
            } => Event::GotInvalidRemotely {
                id: transaction.fetch_id(),
                source,
            },
        }
    }
}

impl<T: FetchItem> Display for Event<T> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Event::Fetch(FetcherRequest { id, .. }) => {
                write!(formatter, "request to fetch item at hash {}", id)
            }
            Event::GetLocallyResult { id, maybe_item, .. } => {
                if maybe_item.is_some() {
                    write!(formatter, "got {} from storage", id)
                } else {
                    write!(formatter, "failed to fetch {} from storage", id)
                }
            }
            Event::GotRemotely { item, source } => {
                write!(formatter, "got {} from {}", item.fetch_id(), source)
            }
            Event::GotInvalidRemotely { id, source } => {
                write!(formatter, "invalid item {} from {}", id, source)
            }
            Event::TimeoutPeer { id, peer } => write!(
                formatter,
                "check get from peer timeout for {} with {}",
                id, peer
            ),
            Event::AbsentRemotely { id, peer } => {
                write!(formatter, "item {} was not available on {}", id, peer)
            }
            Event::RejectedRemotely { id, peer } => {
                write!(
                    formatter,
                    "request to fetch item {} was rejected by {}",
                    id, peer
                )
            }
            Event::PutToStorage { item, .. } => {
                write!(formatter, "item {} was put to storage", item.fetch_id())
            }
        }
    }
}
