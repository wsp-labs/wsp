// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/runtime/src/serve-broker.ts at 65f6b89c7: the broker ops a socket asks, answered off the
//! host's broker. Spec: spec/serve_helpers.md, H24.
use std::rc::Rc;

use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use wsp_wire::wire::place_link::PLACES_TICKET_REFUSAL;
use wsp_wire::wire::runtime::{BrokerStatus, ProviderKeyWorks};
use wsp_wire::wire::runtime_ops::{RuntimeOp, RuntimeRequest};
use wsp_words::exit::{HostError, Kind};

use crate::serve::served::Payload;

pub use wsp_core::sign_in_socket::CloudSignIn;

/// What a host with no broker answers the broker ops with. serve-broker.ts:7.
pub const BROKER_UNSERVED: &str = "the host that answered wired no broker for machines at a cloud provider";

/// The host's broker as its socket answers it; each road absent is refused. serve-broker.ts:9-16.
#[async_trait(?Send)]
pub trait BrokerDoor {
    /// None: the host wired no status road.
    async fn status(&self) -> Option<Result<BrokerStatus, HostError>>;
    /// None: the host wired no key check.
    async fn check_key(&self, provider: &str, key: &str) -> Option<Result<ProviderKeyWorks, HostError>>;
    fn cloud_sign_in(&self) -> Option<CloudSignIn>;
}

/// `providers.checkKey` or `broker.status`. serve-broker.ts:20.
pub fn is_broker_op(msg: &RuntimeRequest) -> bool {
    matches!(msg.op, RuntimeOp::BrokerStatus { .. } | RuntimeOp::ProvidersCheckKey { .. })
}

/// `answerBroker(door, msg, ownRoad)`. serve-broker.ts:22-31.
pub async fn answer_broker(door: Option<&Rc<dyn BrokerDoor>>, msg: &RuntimeRequest, own_road: bool) -> Result<Payload, HostError> {
    match &msg.op {
        RuntimeOp::BrokerStatus { .. } => {
            let status = match door {
                Some(door) => door.status().await,
                None => None,
            };
            match status {
                Some(status) => spread(&status?),
                None => Err(HostError::new(BROKER_UNSERVED)),
            }
        }
        RuntimeOp::ProvidersCheckKey { provider, key, .. } => {
            if !own_road {
                return Err(HostError::with_kind(PLACES_TICKET_REFUSAL, Kind::Ticket));
            }
            let works = match door {
                Some(door) => door.check_key(provider, key).await,
                None => None,
            };
            match works {
                Some(works) => spread(&works?),
                None => Err(HostError::new(BROKER_UNSERVED)),
            }
        }
        _ => Err(HostError::new(format!("{} is not answered here", msg.op.name()))),
    }
}

/// The answer's fields beside id and ok, as the TS spreads them.
fn spread<T: Serialize>(answer: &T) -> Result<Payload, HostError> {
    match serde_json::to_value(answer).map_err(|e| HostError::new(e.to_string()))? {
        Value::Object(fields) => Ok(fields),
        _ => Err(HostError::new("the answer is not an object")),
    }
}
