use async_std::sync::RwLock;
use futures::{future, select};
use async_trait::async_trait;
use std::{
  collections::{HashMap, HashSet, VecDeque},
  net::IpAddr,
};
use uuid::Uuid;

use crate::{
  core::{MessageServer, SpamChecker, MAILBOX_SIZE},
  messages::{
    ClientError, ClientId, ClientMessage, ClientPollReply, ClientReply, FullyQualifiedMessage,
    Sequence, ServerId,
  },
};

use crate::messages::{Outgoing, ServerMessage, ServerReply};

// this structure will contain the data you need to track in your server
// this will include things like delivered messages, clients last seen sequence number, etc.
pub struct Server<C: SpamChecker> {
  checker: C,
  // add things here
}

#[async_trait]
impl<C: SpamChecker + Send + Sync> MessageServer<C> for Server<C> {
  const GROUP_NAME: &'static str = "WRITE YOUR NAMES HERE, NOT YOUR TEAM NAME, YOUR ACTUAL NAMES!";

  fn new(checker: C, id: ServerId) -> Self {
    todo!()
  }

  // note: you need to roll a Uuid, and then convert it into a ClientId
  // Uuid::new_v4() will generate such a value
  // you will most likely have to edit the Server struct as as to store information about the client
  //
  // for spam checking, you will need to run both checks in parallel, and take a decision as soon as
  // each checks return
  async fn register_local_client(&self, src_ip: IpAddr, name: String) -> Option<ClientId> {
    todo!()
  }

  /*
   if the client is known, its last seen sequence number must be verified (and updated)
  */
  async fn handle_sequenced_message<A: Send>(
    &self,
    sequence: Sequence<A>,
  ) -> Result<A, ClientError> {
    todo!()
  }

  /* Here client messages are handled.
    * if the client is local,
      * if the mailbox is full, BoxFull should be returned
      * otherwise, Delivered should be returned
    * if the client is unknown, the message should be stored and Delayed must be returned
    * (federation) if the client is remote, Transfer should be returned

    It is recommended to write an function that handles a single message and use it to handle
    both ClientMessage variants.
  */
  async fn handle_client_message(&self, src: ClientId, msg: ClientMessage) -> Vec<ClientReply> {
    todo!()
  }

  /* for the given client, return the next message or error if available
   */
  async fn client_poll(&self, client: ClientId) -> ClientPollReply {
    todo!()
  }

  /* For announces
     * if the route is empty, return EmptyRoute
     * if not, store the route in some way
     * also store the remote clients
     * if one of these remote clients has messages waiting, return them
    For messages
     * if local, deliver them
     * if remote, forward them
  */
  async fn handle_server_message(&self, msg: ServerMessage) -> ServerReply {
    todo!()
  }

  async fn list_users(&self) -> HashMap<ClientId, String> {
    todo!()
  }

  // return a route to the target server
  // bonus points if it is the shortest route
  async fn route_to(&self, destination: ServerId) -> Option<Vec<ServerId>> {
    todo!()
  }
}

impl<C: SpamChecker + Sync + Send> Server<C> {
  // write your own methods here
}

#[cfg(test)]
mod test {
  use crate::testing::{test_message_server, TestChecker};

  use super::*;

  #[test]
  fn tester() {
    test_message_server::<Server<TestChecker>>();
  }
}
