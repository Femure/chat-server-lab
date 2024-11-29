use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(
  Serialize, Deserialize, std::hash::Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug,
)]
pub struct ClientId(pub(crate) Uuid);
#[derive(
  Serialize, Deserialize, std::hash::Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug,
)]
pub struct ServerId(pub(crate) Uuid);

impl From<u128> for ClientId {
  fn from(value: u128) -> Self {
    ClientId(Uuid::from_u128_le(value))
  }
}

impl From<u128> for ServerId {
  fn from(value: u128) -> Self {
    ServerId(Uuid::from_u128_le(value))
  }
}

impl From<&ClientId> for u128 {
  fn from(value: &ClientId) -> Self {
    value.0.to_u128_le()
  }
}

impl From<&ServerId> for u128 {
  fn from(value: &ServerId) -> Self {
    value.0.to_u128_le()
  }
}

impl Default for ClientId {
  fn default() -> ClientId {
    ClientId(Uuid::new_v4())
  }
}

impl Default for ServerId {
  fn default() -> ServerId {
    ServerId(Uuid::new_v4())
  }
}

impl From<Uuid> for ClientId {
  fn from(value: Uuid) -> Self {
    ClientId(value)
  }
}

impl From<Uuid> for ServerId {
  fn from(value: Uuid) -> Self {
    ServerId(value)
  }
}

impl std::fmt::Display for ClientId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "ClientId({})", self.0)
  }
}

impl std::fmt::Display for ServerId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "ServerId({})", self.0)
  }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Sequence<A> {
  pub seqid: u128,
  pub src: ClientId,
  pub content: A,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum AuthMessage {
  Hello { user: ClientId, nonce: [u8; 8] },
  Nonce { server: ServerId, nonce: [u8; 8] },
  Auth { response: [u8; 16] },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ClientQuery {
  Register(String), // name of the user
  Message(ClientMessage),
  Poll,
  ListUsers,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ClientMessage {
  /// simple text message
  Text { dest: ClientId, content: String },
  /// multiple targets text message
  MText {
    dest: Vec<ClientId>,
    content: String,
  },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FullyQualifiedMessage {
  pub src: ClientId,
  pub srcsrv: ServerId,
  pub dsts: Vec<(ClientId, ServerId)>,
  pub content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ServerMessage {
  /// Servers announcements
  Announce {
    /// The route is the list of servers that were traversed to reach us.
    /// The last element is the closest to us, and the first the farthest.
    route: Vec<ServerId>,
    /// list of clients registed on the source server, with their names
    clients: HashMap<ClientId, String>,
  },
  Message(FullyQualifiedMessage),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ClientError {
  UnknownClient, // client is unknown
  BoxFull(ClientId),
  InternalError,
}

impl std::fmt::Display for ClientError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      ClientError::BoxFull(clientid) => write!(f, "BoxFull({})", clientid),
      ClientError::InternalError => "InternalError".fmt(f),
      ClientError::UnknownClient => "UnknownClient".fmt(f),
    }
  }
}

impl std::error::Error for ClientError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    None
  }

  fn description(&self) -> &str {
    "description() is deprecated; use Display"
  }

  fn cause(&self) -> Option<&dyn std::error::Error> {
    self.source()
  }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ClientReply {
  Delivered,
  Error(ClientError),
  /// unknown recipient, no relays found
  Delayed,
  /// send to an external server
  Transfer(ServerId, ServerMessage),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ClientPollReply {
  Message { src: ClientId, content: String },
  DelayedError(DelayedError),
  Nothing,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum DelayedError {
  UnknownRecipient(ClientId),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Outgoing<A> {
  pub nexthop: ServerId,
  pub message: A,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ServerReply {
  Outgoing(Vec<Outgoing<FullyQualifiedMessage>>),
  EmptyRoute,
  Error(String),
}
