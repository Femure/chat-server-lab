use std::{collections::HashMap, net::IpAddr, time::Duration};

use anyhow::Context;
use async_std::task::sleep;
use async_trait::async_trait;

use crate::{client::Client, core::*, messages::*};

fn localhost() -> IpAddr {
  "127.0.0.1".parse().unwrap()
}

enum TestCheckerMode {
  Standard,
  Set { ip: bool, user: bool },
  DelayIp,
  DelayUser,
}

pub struct TestChecker {
  mode: TestCheckerMode,
}

impl TestChecker {
  fn new(mode: TestCheckerMode) -> Self {
    Self { mode }
  }
}

impl Default for TestChecker {
  fn default() -> Self {
    Self::new(TestCheckerMode::Standard)
  }
}

#[async_trait]
impl SpamChecker for TestChecker {
  async fn is_user_spammer(&self, _name: &str) -> bool {
    match self.mode {
      TestCheckerMode::Standard => false,
      TestCheckerMode::Set { ip: _, user } => user,
      TestCheckerMode::DelayIp => true,
      TestCheckerMode::DelayUser => {
        sleep(Duration::from_secs(10)).await;
        panic!("should not happen, you did not handle spamming checks in parallel")
      }
    }
  }
  async fn is_ip_spammer(&self, _name: &IpAddr) -> bool {
    match self.mode {
      TestCheckerMode::Standard => false,
      TestCheckerMode::Set { ip, user: _ } => ip,
      TestCheckerMode::DelayUser => true,
      TestCheckerMode::DelayIp => {
        sleep(Duration::from_secs(10)).await;
        panic!("should not happen, you did not handle spamming checks in parallel")
      }
    }
  }
}

async fn spammer_both<M: MessageServer<TestChecker>>() -> anyhow::Result<()> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(
    TestChecker::new(TestCheckerMode::Set {
      ip: true,
      user: true,
    }),
    sid,
  );
  if server
    .register_local_client(localhost(), "user1".to_string())
    .await
    .is_some()
  {
    anyhow::bail!("should have been recognized as spammer")
  }
  Ok(())
}

async fn spammer_ip<M: MessageServer<TestChecker>>() -> anyhow::Result<()> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(
    TestChecker::new(TestCheckerMode::Set {
      ip: true,
      user: false,
    }),
    sid,
  );
  if server
    .register_local_client(localhost(), "user1".to_string())
    .await
    .is_some()
  {
    anyhow::bail!("should have been recognized as spammer")
  }
  Ok(())
}

async fn spammer_user<M: MessageServer<TestChecker>>() -> anyhow::Result<()> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(
    TestChecker::new(TestCheckerMode::Set {
      ip: false,
      user: true,
    }),
    sid,
  );
  if server
    .register_local_client(localhost(), "user1".to_string())
    .await
    .is_some()
  {
    anyhow::bail!("should have been recognized as spammer")
  }
  Ok(())
}

async fn spammer_delay_ip<M: MessageServer<TestChecker>>() -> anyhow::Result<()> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(TestChecker::new(TestCheckerMode::DelayIp), sid);
  if server
    .register_local_client(localhost(), "user1".to_string())
    .await
    .is_some()
  {
    anyhow::bail!("should have been recognized as spammer")
  }
  Ok(())
}

async fn spammer_delay_user<M: MessageServer<TestChecker>>() -> anyhow::Result<()> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(TestChecker::new(TestCheckerMode::DelayUser), sid);
  if server
    .register_local_client(localhost(), "user1".to_string())
    .await
    .is_some()
  {
    anyhow::bail!("should have been recognized as spammer")
  }
  Ok(())
}

async fn sequence_correct<M: MessageServer<TestChecker>>() -> Result<(), ClientError> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(TestChecker::default(), sid);
  let c1 = server
    .register_local_client(localhost(), "user1".to_string())
    .await
    .unwrap();
  let c2 = server
    .register_local_client(localhost(), "user2".to_string())
    .await
    .unwrap();
  let mut client1 = Client::new(c1);
  let mut client2 = Client::new(c2);

  // send 1000 messages, correctly sequenced
  for i in 0..100 {
    let message = if i & 1 == 0 {
      client1.sequence(())
    } else {
      client2.sequence(())
    };
    server.handle_sequenced_message(message).await?;
  }
  Ok(())
}

async fn sequence_unknown_user<M: MessageServer<TestChecker>>() -> anyhow::Result<()> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(TestChecker::default(), sid);
  let c1 = ClientId::default();
  let mut client1 = Client::new(c1);

  let message = client1.sequence(());
  match server.handle_sequenced_message(message).await {
    Err(ClientError::UnknownClient) => Ok(()),
    r => anyhow::bail!("Expected Err(UnknownClient), but got {:?}", r),
  }
}

async fn simple_client_test<M: MessageServer<TestChecker>>() -> anyhow::Result<()> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(TestChecker::default(), sid);

  let c1 = server
    .register_local_client(localhost(), "user 1".to_string())
    .await
    .unwrap();
  let c2 = server
    .register_local_client(localhost(), "user 2".to_string())
    .await
    .unwrap();
  let r = server
    .handle_client_message(
      c1,
      ClientMessage::Text {
        dest: c2,
        content: "hello".into(),
      },
    )
    .await;
  if r != [ClientReply::Delivered] {
    anyhow::bail!("expected a single delivered message, got {:?}", r)
  }
  let reply = server.client_poll(c2).await;
  let expected = ClientPollReply::Message {
    src: c1,
    content: "hello".into(),
  };
  if reply != expected {
    anyhow::bail!(
      "Did not receive expected message, expected {:?}, received {:?}",
      expected,
      reply
    );
  }
  Ok(())
}

async fn list_users_test<M: MessageServer<TestChecker>>() -> anyhow::Result<()> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(TestChecker::default(), sid);
  let mut usermap = HashMap::new();
  for n in 0..100_u32 {
    let username = format!("user {n}");
    let id = server
      .register_local_client(localhost(), username.clone())
      .await
      .unwrap();
    usermap.insert(id, username);
  }
  let actual = server.list_users().await;

  if actual != usermap {
    anyhow::bail!("Incorrect user map");
  }
  Ok(())
}

/// sends 100 single messages, and 100 multiple recipients messages
async fn multiple_client_messages_test<M: MessageServer<TestChecker>>() -> anyhow::Result<()> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(TestChecker::default(), sid);

  let c1 = server
    .register_local_client(localhost(), "user 1".to_string())
    .await
    .unwrap();
  let c2 = server
    .register_local_client(localhost(), "user 2".to_string())
    .await
    .unwrap();
  let c3 = server
    .register_local_client(localhost(), "user 3".to_string())
    .await
    .unwrap();
  for i in 0..100 {
    let r = server
      .handle_client_message(
        c1,
        ClientMessage::Text {
          dest: c2,
          content: i.to_string(),
        },
      )
      .await;
    if r != [ClientReply::Delivered] {
      anyhow::bail!("A> Could not deliver message {}, got {:?}", i, r);
    }
  }
  for i in 0..100 {
    let r = server
      .handle_client_message(
        c1,
        ClientMessage::MText {
          dest: vec![c2, c3],
          content: (i + 100).to_string(),
        },
      )
      .await;
    if r != [ClientReply::Delivered, ClientReply::Delivered] {
      anyhow::bail!("B> Could not deliver message {}, got {:?}", i, r);
    }
  }

  for i in 0..200 {
    let reply = server.client_poll(c2).await;
    let expected_reply = ClientPollReply::Message {
      src: c1,
      content: i.to_string(),
    };
    if reply != expected_reply {
      anyhow::bail!(
        "A> Did not receive expected message {}, received {:?}",
        i,
        reply
      );
    }
  }
  for i in 100..200 {
    let reply = server.client_poll(c3).await;
    let expected_reply = ClientPollReply::Message {
      src: c1,
      content: i.to_string(),
    };
    if reply != expected_reply {
      anyhow::bail!(
        "B> Did not receive expected message {}, received {:?}",
        i,
        reply
      );
    }
  }
  let reply = server.client_poll(c2).await;
  if reply != ClientPollReply::Nothing {
    anyhow::bail!(
      "Did not receive the expected Nothing (for client 2) reply, got {:?}",
      reply
    );
  }
  let reply = server.client_poll(c3).await;
  if reply != ClientPollReply::Nothing {
    anyhow::bail!(
      "Did not receive the expected Nothing (for client 3) reply, got {:?}",
      reply
    );
  }
  Ok(())
}

async fn mixed_results_client_message<M: MessageServer<TestChecker>>() -> anyhow::Result<()> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(TestChecker::default(), sid);

  let c1 = server
    .register_local_client(localhost(), "user 1".to_string())
    .await
    .unwrap();
  let c2 = server
    .register_local_client(localhost(), "user 2".to_string())
    .await
    .unwrap();
  let c3 = ClientId::default();

  let m = server
    .handle_client_message(
      c1,
      ClientMessage::MText {
        dest: vec![c2, c3],
        content: "Hello".to_string(),
      },
    )
    .await;
  if m != [ClientReply::Delivered, ClientReply::Delayed] {
    anyhow::bail!("Expected Delivered/Delayed, but got {:?}", m)
  }
  Ok(())
}

async fn mailbox_full<M: MessageServer<TestChecker>>() -> anyhow::Result<()> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(TestChecker::default(), sid);

  let c1 = server
    .register_local_client(localhost(), "user 1".to_string())
    .await
    .unwrap();
  let c2 = server
    .register_local_client(localhost(), "user 2".to_string())
    .await
    .unwrap();

  for n in 0..MAILBOX_SIZE {
    let m = server
      .handle_client_message(
        c1,
        ClientMessage::Text {
          dest: c2,
          content: format!("{n}"),
        },
      )
      .await;
    if m != [ClientReply::Delivered] {
      anyhow::bail!("Expected Delivered, but got {:?}", m)
    }
  }
  let m = server
    .handle_client_message(
      c1,
      ClientMessage::Text {
        dest: c2,
        content: "FULL".into(),
      },
    )
    .await;
  if m != [ClientReply::Error(ClientError::BoxFull(c2))] {
    anyhow::bail!("Expected BoxFull, but got {:?}", m)
  }
  Ok(())
}

async fn message_to_outer_user<M: MessageServer<TestChecker>>() -> anyhow::Result<()> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(TestChecker::default(), sid);

  let c1 = server
    .register_local_client(localhost(), "user 1".to_string())
    .await
    .unwrap();
  let s1 = ServerId::default();
  let s2 = ServerId::default();
  let s3 = ServerId::default();
  let euuid = ClientId::default();

  log::debug!("route: {} -> {} -> {} -> us", s1, s2, s3);

  let r = server
    .handle_server_message(ServerMessage::Announce {
      route: vec![s1, s2, s3],
      clients: HashMap::from([(euuid, "external user".into())]),
    })
    .await;
  if r != ServerReply::Outgoing(Vec::new()) {
    anyhow::bail!("Expected empty outgoing answer, got {:?}", r);
  }
  assert_eq!(r, ServerReply::Outgoing(Vec::new()));
  let r = server
    .handle_client_message(
      c1,
      ClientMessage::Text {
        dest: euuid,
        content: "Hello".to_string(),
      },
    )
    .await;
  let expected = [ClientReply::Transfer(
    s3,
    ServerMessage::Message(FullyQualifiedMessage {
      src: c1,
      srcsrv: sid,
      dsts: vec![(euuid, s1)],
      content: "Hello".to_string(),
    }),
  )];

  if r != expected {
    anyhow::bail!("Expected {:?}\n   , got {:?}", expected, r)
  }

  Ok(())
}

async fn message_to_outer_user_delayed<M: MessageServer<TestChecker>>() -> anyhow::Result<()> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(TestChecker::default(), sid);

  let c1 = server
    .register_local_client(localhost(), "user 1".to_string())
    .await
    .unwrap();
  let s1 = ServerId::default();
  let s2 = ServerId::default();
  let s3 = ServerId::default();
  let euuid = ClientId::default();

  log::debug!("route: {} -> {} -> {} -> us", s1, s2, s3);

  let r = server
    .handle_client_message(
      c1,
      ClientMessage::Text {
        dest: euuid,
        content: "Hello".to_string(),
      },
    )
    .await;
  if r != [ClientReply::Delayed] {
    anyhow::bail!("Expected a delayed message first, but got {:?}", r);
  }
  let r = server
    .handle_server_message(ServerMessage::Announce {
      route: vec![s1, s2, s3],
      clients: HashMap::from([(euuid, "external user".into())]),
    })
    .await;
  let expected = ServerReply::Outgoing(vec![Outgoing {
    nexthop: s3,
    message: FullyQualifiedMessage {
      src: c1,
      srcsrv: sid,
      dsts: vec![(euuid, s1)],
      content: "Hello".to_string(),
    },
  }]);
  if r != expected {
    anyhow::bail!("Expected {:?}\n,    got {:?}", expected, r);
  }

  Ok(())
}

async fn test_route<M: MessageServer<TestChecker>>(
  server: &M,
  dest: ServerId,
  expected: Vec<ServerId>,
) -> anyhow::Result<()> {
  let rt = server.route_to(dest).await;
  if rt.as_ref() != Some(&expected) {
    anyhow::bail!("wrong route: expected {:?}, got {:?}", expected, rt)
  }
  Ok(())
}

async fn routing_test<M: MessageServer<TestChecker>>() -> anyhow::Result<()> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(TestChecker::default(), sid);

  /* map:

       us - s1 - s2
        |         |
       s5 - s4 - s3
  */
  let s1 = ServerId::from(1);
  let s2 = ServerId::from(2);
  let s3 = ServerId::from(3);
  let s4 = ServerId::from(4);
  let s5 = ServerId::from(5);
  let s4_user = ClientId::default();
  // first advertise the long route
  let r = server
    .handle_server_message(ServerMessage::Announce {
      route: vec![s4, s3, s2, s1],
      clients: HashMap::from([(s4_user, "s4 user".into())]),
    })
    .await;
  let expected_empty_out = ServerReply::Outgoing(Vec::new());
  if r != expected_empty_out {
    anyhow::bail!("msg1: Expected {:?}\n,    got {:?}", expected_empty_out, r);
  }
  test_route(&server, s4, vec![sid, s1, s2, s3, s4]).await?;
  // now advertise another alternative route
  let r = server
    .handle_server_message(ServerMessage::Announce {
      route: vec![s2, s3, s4, s5],
      clients: HashMap::new(),
    })
    .await;
  if r != expected_empty_out {
    anyhow::bail!("msg3: Expected {:?}\n,    got {:?}", expected_empty_out, r);
  }
  test_route(&server, s4, vec![sid, s5, s4]).await?;
  Ok(())
}

async fn routing_test2<M: MessageServer<TestChecker>>() -> anyhow::Result<()> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(TestChecker::default(), sid);

  /* map:

       us - s1 - s2 - s6
        |         |    |
       s5 - s4 - s3    |
             |         |
            s7 ---------
  */
  let s1 = ServerId::from(1);
  let s2 = ServerId::from(2);
  let s3 = ServerId::from(3);
  let s4 = ServerId::from(4);
  let s5 = ServerId::from(5);
  let s6 = ServerId::from(6);
  let s7 = ServerId::from(7);
  let s7_user = ClientId::default();
  let r = server
    .handle_server_message(ServerMessage::Announce {
      route: vec![s7, s6, s2, s3, s4, s5],
      clients: HashMap::from([(s7_user, "user".to_string())]),
    })
    .await;
  let expected_empty_out = ServerReply::Outgoing(Vec::new());
  if r != expected_empty_out {
    anyhow::bail!("msg1: Expected {:?}\n,    got {:?}", expected_empty_out, r);
  }
  let r = server
    .handle_server_message(ServerMessage::Announce {
      route: vec![s5, s4, s7, s6, s2, s1],
      clients: HashMap::new(),
    })
    .await;
  let expected_empty_out = ServerReply::Outgoing(Vec::new());
  if r != expected_empty_out {
    anyhow::bail!("msg1: Expected {:?}\n,    got {:?}", expected_empty_out, r);
  }
  test_route(&server, s4, vec![sid, s5, s4])
    .await
    .context("r1")?;
  test_route(&server, s6, vec![sid, s1, s2, s6])
    .await
    .context("r2")?;
  test_route(&server, s7, vec![sid, s5, s4, s7])
    .await
    .context("r3")?;
  Ok(())
}

async fn all_tests<M: MessageServer<TestChecker>>(counter: &mut usize) -> anyhow::Result<()> {
  sequence_correct::<M>()
    .await
    .with_context(|| "sequence_correct")?;
  *counter += 1;
  sequence_unknown_user::<M>()
    .await
    .with_context(|| "sequence_unknown_user")?;
  *counter += 1;
  simple_client_test::<M>()
    .await
    .with_context(|| "simple_client_test")?;
  *counter += 1;
  list_users_test::<M>()
    .await
    .with_context(|| "list_users_test")?;
  *counter += 1;
  multiple_client_messages_test::<M>()
    .await
    .with_context(|| "multiple_client_message_test")?;
  *counter += 1;
  mixed_results_client_message::<M>()
    .await
    .with_context(|| "mixed_results_client_message")?;
  *counter += 1;
  mailbox_full::<M>().await.with_context(|| "mailbox_full")?;
  *counter += 1;
  spammer_ip::<M>().await.with_context(|| "spammer_ip")?;
  *counter += 1;
  spammer_user::<M>().await.with_context(|| "spammer_user")?;
  *counter += 1;
  spammer_both::<M>().await.with_context(|| "spammer_both")?;
  *counter += 1;
  message_to_outer_user::<M>()
    .await
    .with_context(|| "message_to_outer_user")?;
  *counter += 1;
  message_to_outer_user_delayed::<M>()
    .await
    .with_context(|| "message_to_outer_user_delayed")?;
  *counter += 1;
  spammer_delay_ip::<M>()
    .await
    .with_context(|| "spammer_delay_ip")?;
  *counter += 1;
  spammer_delay_user::<M>()
    .await
    .with_context(|| "spammer_delay_user")?;
  *counter += 1;
  routing_test::<M>().await.with_context(|| "real routing")?;
  *counter += 1;
  routing_test2::<M>()
    .await
    .with_context(|| "real routing 2")?;
  *counter += 1;
  Ok(())
}

pub(crate) fn test_message_server<M: MessageServer<TestChecker>>() {
  pretty_env_logger::init();
  async_std::task::block_on(async {
    let mut counter = 0;
    match all_tests::<M>(&mut counter).await {
      Ok(()) => (),
      Err(rr) => panic!("counter={}, error={:?}", counter, rr),
    }
  });
}
