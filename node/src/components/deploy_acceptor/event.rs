use std::fmt::{self, Display, Formatter};

use serde::Serialize;

use super::Source;
use crate::{
    components::deploy_acceptor::Error,
    effect::{announcements::RpcServerAnnouncement, Responder},
    types::{Deploy, NodeId},
};
use casper_types::Key;

/// `DeployAcceptor` events.
#[derive(Debug, Serialize)]
pub(crate) enum Event {
    /// The initiating event to accept a new `Deploy`.
    Accept {
        deploy: Box<Deploy>,
        source: Source<NodeId>,
        responder: Option<Responder<Result<(), Error>>>,
    },
    // /// The result of the `DeployAcceptor` putting a `Deploy` to the storage component.
    // PutToStorageResult {
    //     deploy: Box<Deploy>,
    //     source: Source<NodeId>,
    //     is_new: bool,
    // },
    /// The InvalidDeployResult event
    InvalidDeployResult {
        deploy: Box<Deploy>,
        source: Source<NodeId>,
        error: Error,
        maybe_responder: Option<Responder<Result<(), Error>>>,
    },
}

impl From<RpcServerAnnouncement> for Event {
    fn from(announcement: RpcServerAnnouncement) -> Self {
        match announcement {
            RpcServerAnnouncement::DeployReceived { deploy, responder } => Event::Accept {
                deploy,
                source: Source::<NodeId>::Client,
                responder,
            },
        }
    }
}

impl Display for Event {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Event::Accept { deploy, source, .. } => {
                write!(formatter, "accept {} from {}", deploy.id(), source)
            }
            // Event::PutToStorageResult { deploy, is_new, .. } => {
            //     if *is_new {
            //         write!(formatter, "put new {} to storage", deploy.id())
            //     } else {
            //         write!(formatter, "had already stored {}", deploy.id())
            //     }
            // }
            Event::InvalidDeployResult { .. } => {}
        }
    }
}
