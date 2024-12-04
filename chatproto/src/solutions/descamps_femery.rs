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

// This structure represents the server with the required data to track clients and messages.
// It includes clients' messages, last seen sequence number, and other necessary data.
pub struct Server<C: SpamChecker> {
  checker: C,   // The spam checker used for client registration.
  id: ServerId, // Unique server identifier.
  clients: RwLock<HashMap<ClientId, Client>>, // A hashmap to store local clients.
  routes: RwLock<Vec<Vec<ServerId>>>, // Routes between servers.
  remote_clients: RwLock<HashMap<ClientId, RemoteClient>>, // A hashmap of remote clients.
  stored_messages: RwLock<HashMap<ClientId, Message>>, // Stored messages for remote clients.
}

// Represents a local client with its IP, name, sequence ID, and mailbox for storing messages.
struct Client {
  _src_ip: IpAddr,                       // Source IP address of the client.
  name: String,                          // Name of the client.
  seqid: u128,                           // Last seen sequence ID.
  mailbox: VecDeque<(ClientId, String)>, // Mailbox to store messages for the client.
}

// Represents a remote client with its name and associated server.
struct RemoteClient {
  _name: String,    // Name of the remote client.
  srcsrv: ServerId, // Server ID that the remote client is connected to.
}

// A message sent by a client, consisting of the sender's ID and content.
struct Message {
  src: ClientId,   // Client ID of the sender.
  content: String, // Content of the message.
}

#[async_trait]
impl<C: SpamChecker + Send + Sync> MessageServer<C> for Server<C> {
  const GROUP_NAME: &'static str = "Descamps Femery"; // The group name for the server.

  // Initializes a new server with the given spam checker and server ID.
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

  // Registers a local client by checking if the client's IP and name are flagged as spammers.
  // Returns a ClientId if the client is successfully registered.
  async fn register_local_client(&self, src_ip: IpAddr, name: String) -> Option<ClientId> {
    let spam_check_timeout = Duration::from_secs(2); // Timeout duration for spam checks.

    // Run both spam checks concurrently using the join! macro.
    let (is_ip_spammer, is_user_spammer) = join!(
      timeout(spam_check_timeout, self.checker.is_ip_spammer(&src_ip)),
      timeout(spam_check_timeout, self.checker.is_user_spammer(&name)),
    );

    match (is_ip_spammer, is_user_spammer) {
      (Ok(ip_result), Ok(user_result)) => {
        // Only proceed if neither the IP nor the user is flagged as a spammer.
        if !ip_result && !user_result {
          let client = ClientId(Uuid::new_v4()); // Generate a new ClientId.
          let client_info = Client {
            _src_ip: src_ip,
            name,
            seqid: 0,
            mailbox: VecDeque::new(),
          };
          self.clients.write().await.insert(client, client_info); // Insert the client into the server.
          return Some(client); // Return the generated ClientId.
        }
      }
      _ => {
        // Handle timeout or error in spam checks.
        eprintln!("Spam check timeout or error occurred");
      }
    }
    None // Return None if the client is not registered.
  }

  // Handles a sequenced message from a client, verifies the client's sequence ID, and updates it.
  async fn handle_sequenced_message<A: Send>(
    &self,
    sequence: Sequence<A>,
  ) -> Result<A, ClientError> {
    let mut clients = self.clients.write().await; // Acquire write lock on clients.
    let client = clients.get_mut(&sequence.src); // Find the client by its ID.
    match client {
      Some(client) => {
        // If the client's sequence ID is less than the received one, update it and return the content.
        if client.seqid < sequence.seqid {
          client.seqid = sequence.seqid;
          Ok(sequence.content)
        } else {
          // If the sequence ID is not newer, return an error.
          Err(ClientError::InternalError)
        }
      }
      None => Err(ClientError::UnknownClient), // Return an error if the client is unknown.
    }
  }

  // Handles incoming client messages. Depending on whether the client is local or remote,
  // the message is either delivered, delayed, or forwarded.
  async fn handle_client_message(&self, src: ClientId, msg: ClientMessage) -> Vec<ClientReply> {
    let mut resp = Vec::new();
    match msg {
      ClientMessage::Text { dest, content } => {
        // Handle a single destination for a text message.
        resp.push(self.client_message(src, dest, content).await);
      }
      ClientMessage::MText { dest, content } => {
        // Handle multiple destinations for a text message.
        for dst in dest {
          resp.push(self.client_message(src, dst, content.clone()).await);
        }
      }
    }
    resp
  }

  // Polls for the next message for a given client. If no message is available, returns Nothing.
  async fn client_poll(&self, client: ClientId) -> ClientPollReply {
    let mut clt = self.clients.write().await; // Acquire write lock on clients.
    let clt = clt.get_mut(&client); // Find the client by its ID.
    match clt {
      Some(clt) => {
        // If the client has messages in its mailbox, return the first one.
        let (src, content) = match clt.mailbox.pop_front() {
          Some(value) => value,
          None => return ClientPollReply::Nothing, // Return Nothing if no messages are available.
        };
        return ClientPollReply::Message { src, content }; // Return the message.
      }
      None => return ClientPollReply::DelayedError(DelayedError::UnknownRecipient(client)), // Error if the client is not found.
    }
  }

  // Handles server messages. It processes announcements, stores routes, and manages message forwarding.
  async fn handle_server_message(&self, msg: ServerMessage) -> ServerReply {
    match msg {
      ServerMessage::Announce { route, clients } => {
        if route.is_empty() {
          return ServerReply::EmptyRoute; // Return EmptyRoute if the route is empty.
        } else {
          // Store the route and handle remote client announcements.
          self.routes.write().await.push(route.clone()); // Store the route.
          let srv_dst = self.get_srv_dist(&route); // Get the destination server.
          let nexthop = self.get_nexthop(&route); // Get the next hop for routing.

          let mut resp = Vec::new();
          for (client_dst, name) in clients {
            // Store each remote client in the remote_clients hashmap.
            self.remote_clients.write().await.insert(
              client_dst,
              RemoteClient {
                _name: name.clone(),
                srcsrv: srv_dst,
              },
            );

            // If any remote client has stored messages, prepare them for delivery.
            if let Some(message) = self.stored_messages.write().await.remove(&client_dst) {
              resp.push(Outgoing {
                nexthop,
                message: FullyQualifiedMessage {
                  src: message.src,
                  srcsrv: self.id,
                  dsts: vec![(client_dst, srv_dst)],
                  content: message.content.clone(),
                },
              });
            }
          }
          ServerReply::Outgoing(resp) // Return the outgoing messages.
        }
      }
      ServerMessage::Message(fully_qualified_message) => {
        // Handle incoming messages, either delivering them locally or forwarding them remotely.
        if let Some((client_dst, server_dst)) =
          fully_qualified_message.dsts.clone().into_iter().next()
        {
          if let Some(info) = self.clients.write().await.get_mut(&client_dst) {
            // If the client is local, deliver the message.
            info.mailbox.push_back((
              fully_qualified_message.src,
              fully_qualified_message.content.clone(),
            ));
          }

          // Find the route to the destination server.
          let route = match self.route_to(server_dst).await {
            Some(value) => value,
            None => return ServerReply::Error("Route for the client not found".to_string()),
          };

          let nexthop = self.get_nexthop(&route); // Get the next hop for routing.
          return ServerReply::Outgoing(vec![Outgoing {
            nexthop,
            message: fully_qualified_message,
          }]);
        }
        ServerReply::Error("No destination found for the message".to_string()) // Error if no destination found.
      }
    }
  }

  // Lists all registered clients with their names.
  async fn list_users(&self) -> HashMap<ClientId, String> {
    let client_guard = self.clients.read().await; // Acquire read lock on clients.
    client_guard
      .iter()
      .map(|(id, client)| (*id, client.name.clone())) // Map each client to its name.
      .collect()
  }

  // Finds a route to the target server using a graph and breadth-first search (BFS).
  async fn route_to(&self, destination: ServerId) -> Option<Vec<ServerId>> {
    let mut graph: HashMap<ServerId, Vec<ServerId>> = HashMap::new();

    // Step 1: Build the graph from the stored routes.
    for route in self.routes.read().await.iter() {
      for window in route.windows(2) {
        let (a, b) = (window[0], window[1]);
        graph.entry(a).or_default().push(b);
        graph.entry(b).or_default().push(a); // Bidirectional edges.
      }
      // Connect the self server to the nearest server in each route.
      if let Some(&first_server) = route.last() {
        graph.entry(self.id).or_default().push(first_server);
        graph.entry(first_server).or_default().push(self.id); // Bidirectional.
      }
    }

    // Step 2: Perform BFS to find the shortest path.
    let mut queue = VecDeque::new();
    let mut visited = HashMap::new(); // Track visited servers and their predecessors.
    queue.push_back(self.id); // Start from the current server.
    visited.insert(self.id, None); // Mark the current server as visited.

    while let Some(current) = queue.pop_front() {
      if current == destination {
        // Step 3: Reconstruct the path from destination to source.
        let mut path = Vec::new();
        let mut node = Some(current);
        while let Some(n) = node {
          path.push(n);
          node = visited.get(&n).and_then(|&v| v); // Follow the predecessors.
        }
        path.reverse(); // Reverse the path to get the correct order.
        return Some(path); // Return the path.
      }

      // Add neighbors to the queue.
      if let Some(neighbors) = graph.get(&current) {
        for &neighbor in neighbors {
          visited.entry(neighbor).or_insert_with(|| {
            queue.push_back(neighbor);
            Some(current) // Track the predecessor.
          });
        }
      }
    }

    None // Return None if no path is found.
  }
}

impl<C: SpamChecker + Sync + Send> Server<C> {
  
  // Handles sending a message from a client (src) to another client (dest) with the given content.
  // Returns a ClientReply depending on whether the message is delivered, transferred, delayed, or if there was an error.
  async fn client_message(&self, src: ClientId, dest: ClientId, content: String) -> ClientReply {
    // Lock the clients map for mutable access.
    let mut client = self.clients.write().await;

    // Check if the destination client exists locally.
    let client = client.get_mut(&dest);
    match client {
      // If the destination client is local:
      Some(client) => {
        if client.mailbox.len() == MAILBOX_SIZE {
          // If the client's mailbox is full, return an error (BoxFull).
          ClientReply::Error(ClientError::BoxFull(dest))
        } else {
          // Otherwise, add the message to the client's mailbox and return Delivered.
          client.mailbox.push_back((src, content));
          ClientReply::Delivered
        }
      }
      None => {
        // If the client is not found locally, check if it's a remote client.
        let remote_client = self.remote_clients.write().await;
        match remote_client.get(&dest) {
          // If the destination is a remote client:
          Some(client_remote_info) => {
            // Check the routes to find the correct destination server.
            for route in self.routes.read().await.iter() {
              // Get the destination server and the next hop from the route.
              let srv_dst = self.get_srv_dist(route);
              let nexthop = self.get_nexthop(route);

              // If the destination server matches the remote client's server:
              if srv_dst == client_remote_info.srcsrv {
                // Create a server message to transfer the message.
                let message = ServerMessage::Message(FullyQualifiedMessage {
                  src,
                  srcsrv: self.id,
                  dsts: vec![(dest, srv_dst)],  // Destination client and server.
                  content: content.clone(),
                });

                // Return a transfer reply with the next hop and the message.
                return ClientReply::Transfer(nexthop, message);
              }
            }
            // If no matching server is found, return an error (UnknownClient).
            ClientReply::Error(ClientError::UnknownClient)
          }
          // If the destination client is not found at all, store the message for later delivery.
          None => {
            self
              .stored_messages
              .write()
              .await
              .insert(dest, Message { src, content });

            // Return a delayed reply to indicate the message will be sent later.
            ClientReply::Delayed
          }
        }
      }
    }
  }

  // Retrieves the destination server ID from a route (the first server in the route).
  fn get_srv_dist(&self, route: &[ServerId]) -> ServerId {
    *route.first().unwrap()  // Return the first server ID from the route.
  }

  // Retrieves the next hop server ID from a route (the last server in the route).
  fn get_nexthop(&self, route: &[ServerId]) -> ServerId {
    *route.last().unwrap()  // Return the last server ID from the route.
  }
}

// Tests for the server implementation.
#[cfg(test)]
mod test {
  use crate::testing::{test_message_server, TestChecker};

  use super::*;

  // Test case for the message server, using a test checker to simulate message handling.
  #[test]
  fn tester() {
    // Run the test for the message server with the test checker.
    test_message_server::<Server<TestChecker>>();
  }
}
