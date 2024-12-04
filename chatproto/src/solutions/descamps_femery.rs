use async_std::{future::timeout, sync::RwLock};
use async_trait::async_trait;
use futures::join;
use std::{
  collections::{HashMap, VecDeque},
  net::IpAddr,
  time::Duration,
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
  routes: RwLock<Vec<Vec<ServerId>>>,
  remote_clients: RwLock<HashMap<ClientId, RemoteClient>>,
  stored_messages: RwLock<HashMap<ClientId, Message>>,
}

struct Client {
  _src_ip: IpAddr,
  name: String,
  seqid: u128,
  mailbox: VecDeque<(ClientId, String)>,
}

struct RemoteClient {
  _name: String,
  srcsrv: ServerId,
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
      routes: RwLock::new(Vec::new()),
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
    // timeout for the spam checks
    let spam_check_timeout = Duration::from_secs(2);

    let (is_ip_spammer, is_user_spammer) = join!(
      timeout(spam_check_timeout, self.checker.is_ip_spammer(&src_ip)),
      timeout(spam_check_timeout, self.checker.is_user_spammer(&name)),
    );

    match (is_ip_spammer, is_user_spammer) {
      (Ok(ip_result), Ok(user_result)) => {
        // Only proceed if neither the IP nor the user is flagged as a spammer
        if !ip_result && !user_result {
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
      }
      _ => {
        // Handle timeout or error in spam checks (e.g., log a warning, or just return None)
        eprintln!("Spam check timeout or error occurred");
      }
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
          // If not, store the route in some way associated from client_dst and the route
          self.routes.write().await.push(route.clone());

          let srv_dst = self.get_srv_dist(&route);
          let nexthop = self.get_nexthop(&route);

          // On ajoute à la liste chaque message stored pour le client distant
          let mut resp = Vec::new();

          for (client_dst, name) in clients {
            // On enregistre chaque client distant avec leur par leur ID client associé avec leur nom
            // Store the remote clients
            self.remote_clients.write().await.insert(
              client_dst,
              RemoteClient {
                _name: name.clone(),
                srcsrv: srv_dst,
              },
            );

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
        if let Some((client_dst, server_dst)) =
          fully_qualified_message.dsts.clone().into_iter().next()
        {
          // Si le client distant correspond à client local on délivre le message
          if let Some(info) = self.clients.write().await.get_mut(&client_dst) {
            info.mailbox.push_back((
              fully_qualified_message.src,
              fully_qualified_message.content.clone(),
            ));
          }

          // La route qui mène au client distant
          let route = match self.route_to(server_dst).await {
            Some(value) => value,
            None => return ServerReply::Error("Route for the client not found".to_string()),
          };

          let nexthop = self.get_nexthop(&route);

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
      .map(|(id, client)| (*id, client.name.clone()))
      .collect()
  }

  // return a route to the target server
  // bonus points if it is the shortest route
  async fn route_to(&self, destination: ServerId) -> Option<Vec<ServerId>> {
    let mut graph: HashMap<ServerId, Vec<ServerId>> = HashMap::new();

    // Step 1: Build the graph
    for route in self.routes.read().await.iter() {
      for window in route.windows(2) {
        let (a, b) = (window[0], window[1]);
        graph.entry(a).or_default().push(b);
        graph.entry(b).or_default().push(a); // Bidirectional edge
      }
      // Connect the self server to the nearest server in each route
      if let Some(&first_server) = route.last() {
        graph.entry(self.id).or_default().push(first_server);
        graph.entry(first_server).or_default().push(self.id); // Bidirectional
      }
    }

    // Step 2: BFS to find the shortest path
    let mut queue = VecDeque::new();
    let mut visited = HashMap::new(); // Track visited servers and their predecessors
    queue.push_back(self.id);
    visited.insert(self.id, None);

    while let Some(current) = queue.pop_front() {
      if current == destination {
        // Step 3: Reconstruct the path
        let mut path = Vec::new();
        let mut node = Some(current);
        while let Some(n) = node {
          path.push(n);
          node = visited.get(&n).and_then(|&v| v);
        }
        path.reverse();
        return Some(path);
      }

      // Add neighbors to the queue
      if let Some(neighbors) = graph.get(&current) {
        for &neighbor in neighbors {
          visited.entry(neighbor).or_insert_with(|| {
            queue.push_back(neighbor);
            Some(current) // Track the predecessor
          });
        }
      }
    }

    None // No path found
  }
}

impl<C: SpamChecker + Sync + Send> Server<C> {
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
          Some(client_remote_info) => {
            for route in self.routes.read().await.iter() {
              let srv_dst = self.get_srv_dist(route);
              let nexthop = self.get_nexthop(route);

              if srv_dst == client_remote_info.srcsrv {
                let message = ServerMessage::Message(FullyQualifiedMessage {
                  src,
                  srcsrv: self.id,
                  dsts: vec![(dest, srv_dst)],
                  content: content.clone(),
                });
                return ClientReply::Transfer(nexthop, message);
              }
            }
            ClientReply::Error(ClientError::UnknownClient)
          }
          // if the client is unknown, the message should be stored and Delayed must be returned (federation)
          None => {
            self
              .stored_messages
              .write()
              .await
              .insert(dest, Message { src, content });
            ClientReply::Delayed
          }
        }
      }
    }
  }

  // Le serveur distant correspond au premier serveur ID de la route
  fn get_srv_dist(&self, route: &[ServerId]) -> ServerId {
    *route.first().unwrap()
  }

  // Le nexthop correspond au premier serveur ID de la route
  fn get_nexthop(&self, route: &[ServerId]) -> ServerId {
    *route.last().unwrap()
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
