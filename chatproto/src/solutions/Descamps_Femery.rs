use async_std::sync::RwLock;
use async_trait::async_trait;
use futures::join;
use std::{
  collections::{HashMap, VecDeque},
  net::IpAddr,
};
use uuid::Uuid;

use crate::{
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
  clients: RwLock<HashMap<ClientId, Client>>,
  routes: RwLock<HashMap<ClientId, Vec<ServerId>>>,
  remote_clients: RwLock<HashMap<ClientId, String>>,
  stored_messages: RwLock<HashMap<ClientId, Message>>,
}

struct Client {
  _src_ip: IpAddr,
  name: String,
  seqid: u128,
  mailbox: VecDeque<(ClientId, String)>,
}

struct RemoteClient{
  name: String,
  srcsrv: ServerId
}

struct Message {
  src: ClientId,
  content: String,
}

#[async_trait]
impl<C: SpamChecker + Send + Sync> MessageServer<C> for Server<C> {
  const GROUP_NAME: &'static str = "Descamps Femery";

  fn new(checker: C, id: ServerId) -> Self {
    Server {
      checker,
      id,
      clients: RwLock::new(HashMap::new()),
      routes: RwLock::new(HashMap::new()),
      remote_clients: RwLock::new(HashMap::new()),
      stored_messages: RwLock::new(HashMap::new()),
    }
  }

  // note: you need to roll a Uuid, and then convert it into a ClientId
  // Uuid::new_v4() will generate such a value
  // you will most likely have to edit the Server struct as as to store information about the client
  //
  // for spam checking, you will need to run both checks in parallel, and take a decision as soon as
  // each checks return

  async fn register_local_client(&self, src_ip: IpAddr, name: String) -> Option<ClientId> {
    // Run spam checks in parallel
    let (is_ip_spammer, is_user_spammer) = join!(
      self.checker.is_ip_spammer(&src_ip),
      self.checker.is_user_spammer(&name),
    );

    // Only proceed if neither the IP nor the user is flagged as a spammer
    if !is_ip_spammer && !is_user_spammer {
      let client = ClientId(Uuid::new_v4());
      let client_info = Client {
        _src_ip: src_ip,
        name,
        seqid: 0,
        mailbox: VecDeque::new(),
      };
      self.clients.write().await.insert(client, client_info);
      return Some(client);
    }
    None
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
    let mut resp = Vec::new();
    match msg {
      ClientMessage::Text { dest, content } => {
        resp.push(self.client_message(src, dest, content).await);
      }
      ClientMessage::MText { dest, content } => {
        for dst in dest {
          resp.push(self.client_message(src, dst, content.clone()).await)
        }
      }
    }
    resp
  }

  /* for the given client, return the next message or error if available
   */
  async fn client_poll(&self, client: ClientId) -> ClientPollReply {
    let mut clt = self.clients.write().await;
    let clt = clt.get_mut(&client);
    match clt {
      Some(clt) => {
        let (src, content) = match clt.mailbox.pop_front() {
          Some(value) => value,
          None => return ClientPollReply::Nothing,
        };
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
    match msg {
      ServerMessage::Announce { route, clients } => {
        if route.is_empty() {
          return ServerReply::EmptyRoute;
        } else {
          // Le serveur distant correspond au premier serveur ID de la route
          let srv_dst = route.first().unwrap().clone();

          // Le nexthop correspond au dernier serveur ID de la route
          let nexthop = route.last().unwrap().clone();

          // On ajoute à la liste chaque message stored pour le client distant
          let mut resp = Vec::new();

          for (client_dst, name) in clients {
            // On enregistre chaque client distant avec leur par leur ID client associé avec leur nom
            // Store the remote clients
            self
              .remote_clients
              .write()
              .await
              .insert(client_dst, name.clone());

            // If not, store the route in some way associated from client_dst and the route
            self.routes.write().await.insert(client_dst, route.clone());

            // if one of these remote clients has messages waiting, return them
            if let Some(message) = self.stored_messages.write().await.remove(&client_dst) {
              resp.push(Outgoing {
                nexthop,
                message: FullyQualifiedMessage {
                  // Client source
                  src: message.src,
                  // Serveur source
                  srcsrv: self.id,
                  // Liste des serveurs distants avec leurs clients
                  dsts: vec![(client_dst, srv_dst)],
                  // Message texte envoyé
                  content: message.content.clone(),
                },
              })
            }
          }
          ServerReply::Outgoing(resp)
        }
      }
      ServerMessage::Message(fully_qualified_message) => {
        // Le client distant

        for (client_dst, server_dst) in fully_qualified_message.dsts.clone() {
          // Si le client distant correspond à client local on délivre le message
          let mut clients = self.clients.write().await;
          if let Some(info) = clients.get_mut(&client_dst) {
            info.mailbox.push_back((
              fully_qualified_message.src.clone(),
              fully_qualified_message.content.clone(),
            ));
          }

          // La route qui mène au client distant
          let route = match self.route_to(server_dst).await {
            Some(value) => value,
            None => return ServerReply::Error("Route for the client not found".to_string()),
          };

          // Le nexthop correspond au premier serveur ID de la route
          let nexthop = route.first().unwrap().clone();
          return ServerReply::Outgoing(vec![Outgoing {
            nexthop,
            message: fully_qualified_message,
          }]);
        }
        ServerReply::Error("No destination found for the message".to_string())
      }
    }
  }

  async fn list_users(&self) -> HashMap<ClientId, String> {
    let client_guard = self.clients.read().await;
    client_guard
      .iter()
      .map(|(id, client)| (id.clone(), client.name.clone()))
      .collect()
  }

  // return a route to the target server
  // bonus points if it is the shortest route
  async fn route_to(&self, destination: ServerId) -> Option<Vec<ServerId>> {
    for (_, routes) in self.routes.read().await.iter() {
      if routes.first().unwrap() == &destination {
        let mut new_route = routes.clone();
        new_route.push(self.id);
        return Some(new_route.iter().rev().cloned().collect());
      }
    }
    return None;
  }
}

impl<C: SpamChecker + Sync + Send> Server<C> {
  // write your own methods here

  async fn client_message(&self, src: ClientId, dest: ClientId, content: String) -> ClientReply {
    let mut client = self.clients.write().await;
    let client = client.get_mut(&dest);
    match client {
      // if the client is local
      Some(client) => {
        if client.mailbox.len() == MAILBOX_SIZE {
          // if the mailbox is full, BoxFull should be returned
          ClientReply::Error(ClientError::BoxFull(dest))
        } else {
          // otherwise, Delivered should be returned
          client.mailbox.push_back((src, content));
          ClientReply::Delivered
        }
      }
      None => {
        let remote_client = self.remote_clients.write().await;
        match remote_client.get(&dest) {
          // if the client is remote, Transfer should be returned
          Some(_) => {
            let routes = self.routes.read().await;
            let route = match routes.get(&dest) {
              Some(value) => value,
              None => return ClientReply::Error(ClientError::UnknownClient),
            };

            // Le serveur distant correspond au premier serveur ID de la route
            let srv_dst = route.first().unwrap().clone();

            // Le nexthop correspond au dernier serveur ID de la route
            let nexthop = route.last().unwrap().clone();

            let message = ServerMessage::Message(FullyQualifiedMessage {
              src,
              srcsrv: self.id,
              dsts: vec![(dest, srv_dst)],
              content,
            });

            return ClientReply::Transfer(nexthop, message);
          }
          // if the client is unknown, the message should be stored and Delayed must be returned (federation)
          None => {
            self
              .stored_messages
              .write()
              .await
              .insert(dest, Message { src, content });
            return ClientReply::Delayed;
          }
        };
      }
    }
  }
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
