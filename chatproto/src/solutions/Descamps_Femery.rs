use async_std::sync::RwLock;
use async_trait::async_trait;
use futures::{future, select};
use std::{
  collections::{HashMap, HashSet, VecDeque},
  net::IpAddr,
};
use uuid::Uuid;

use crate::{
  client,
  core::{MessageServer, SpamChecker, MAILBOX_SIZE},
  messages::{
    ClientError, ClientId, ClientMessage, ClientPollReply, ClientReply, DelayedError,
    FullyQualifiedMessage, Sequence, ServerId,
  },
};

use crate::messages::{Outgoing, ServerMessage, ServerReply};

// this structure will contain the data you need to track in your server
// this will include things like delivered messages, clients last seen sequence number, etc.
pub struct Server<C: SpamChecker> {
  checker: C,
  id: ServerId,
  clients: RwLock<HashMap<ClientId, ClientInfo>>,
  // add things here
}

struct ClientInfo {
  src_ip: IpAddr,
  name: String,
  seqid: u128,
  mailbox_size: usize,
  mailbox: Vec<(ClientId, String)>,
}

#[async_trait]
impl<C: SpamChecker + Send + Sync> MessageServer<C> for Server<C> {
  const GROUP_NAME: &'static str = "Descamps Femery";

  fn new(checker: C, id: ServerId) -> Self {
    Server {
      checker,
      id,
      clients: RwLock::new(HashMap::new()),
    }
  }

  // note: you need to roll a Uuid, and then convert it into a ClientId
  // Uuid::new_v4() will generate such a value
  // you will most likely have to edit the Server struct as as to store information about the client
  //
  // for spam checking, you will need to run both checks in parallel, and take a decision as soon as
  // each checks return
  async fn register_local_client(&self, src_ip: IpAddr, name: String) -> Option<ClientId> {
    let client = ClientId(Uuid::new_v4());
    let client_info = ClientInfo {
      src_ip,
      name,
      seqid: 0,
      mailbox_size: 0,
      mailbox: Vec::new(),
    };
    self.clients.write().await.insert(client, client_info);
    Some(client)
  }

  /*
   if the client is known, its last seen sequence number must be verified (and updated)
  */
  //pas read and write imbriqué
  async fn handle_sequenced_message<A: Send>(
    &self,
    sequence: Sequence<A>,
  ) -> Result<A, ClientError> {
    let mut clients = self.clients.write().await;
    let client = clients.get_mut(&sequence.src);
    match client {
      Some(client) => {
        if client.seqid < sequence.seqid {
          client.seqid = sequence.seqid;
          Ok(sequence.content)
        } else {
          Err(ClientError::InternalError)
        }
      }
      None => Err(ClientError::UnknownClient),
    }
  }

  /* Here client messages are handled.
    * if the client is local,
      * if the mailbox is full, BoxFull should be returned
      * otherwise, Delivered should be returned
    * if the client is unknown, the message should be stored and Delayed must be returned (federation)
    * if the client is remote, Transfer should be returned

    It is recommended to write an function that handles a single message and use it to handle
    both ClientMessage variants.
  */
  async fn handle_client_message(&self, src: ClientId, msg: ClientMessage) -> Vec<ClientReply> {
    match msg {
      ClientMessage::Text { dest, content } => {
        let mut client = self.clients.write().await;
        let client = client.get_mut(&dest);
        match client {
          Some(client) => {
            if client.mailbox_size == MAILBOX_SIZE {
              vec![ClientReply::Error(ClientError::BoxFull(src))]
            } else {
              client.mailbox_size += 1;
              client.mailbox.push((src, content));
              vec![ClientReply::Delivered]
            }
          }
          None => {
            vec![ClientReply::Delayed]
          }
        }
      }
      ClientMessage::MText { dest, content } => {
        let mut resp = vec![];
        for dst in dest {
          let mut client = self.clients.write().await;
          let client = client.get_mut(&dst);
          match client {
            Some(client) => {
              if client.mailbox_size == MAILBOX_SIZE {
                resp.push(ClientReply::Error(ClientError::BoxFull(src)));
              } else {
                client.mailbox_size += 1;
                client.mailbox.push((src, content.clone()));
                resp.push(ClientReply::Delivered);
              }
            }
            None => resp.push(ClientReply::Delayed),
          }
        }
        resp
      }
    }
  }

  /* for the given client, return the next message or error if available
   */
  async fn client_poll(&self, client: ClientId) -> ClientPollReply {
    let mut clt = self.clients.write().await;
    let clt = clt.get_mut(&client);
    match clt {
      Some(clt) => {
        let (src, content) = match clt.mailbox.pop() {
          Some(value) => value,
          None => return ClientPollReply::Nothing,
        };
        clt.mailbox_size -= 1;
        return ClientPollReply::Message { src, content };
      }
      None => return ClientPollReply::DelayedError(DelayedError::UnknownRecipient(client)),
    }
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
