pub mod decode;
pub mod encode;

#[cfg(test)]
mod test {
  use std::collections::HashMap;
  use std::io::Cursor;
  use uuid::uuid;

  use crate::messages::*;

  use super::decode;
  use super::encode;

  fn servermessages() -> Vec<ServerMessage> {
    // large announce
    vec![
      ServerMessage::Announce {
        route: vec![ServerId::default()],
        clients: HashMap::from([(ClientId::default(), "Roger".to_string())]),
      },
      ServerMessage::Announce {
        route: vec![ServerId::default(), ServerId::default()],
        clients: HashMap::from([
          (ClientId::default(), "user 1".to_string()),
          (ClientId::default(), "user 2".to_string()),
        ]),
      },
      ServerMessage::Announce {
        route: (0..4000).map(|_| ServerId::default()).collect::<Vec<_>>(),
        clients: (0..6000)
          .map(|_| (ClientId::default(), "same name".to_string()))
          .collect::<HashMap<_, _>>(),
      },
      ServerMessage::Message(FullyQualifiedMessage {
        src: ClientId::default(),
        srcsrv: ServerId::default(),
        dsts: vec![(ClientId::default(), ServerId::default())],
        content: "Hello".into(),
      }),
      ServerMessage::Message(FullyQualifiedMessage {
        src: ClientId::default(),
        srcsrv: ServerId::default(),
        dsts: vec![
          (ClientId::default(), ServerId::default()),
          (ClientId::default(), ServerId::default()),
        ],
        content: "World!".into(),
      }),
    ]
  }

  fn server_hardcoded() -> Vec<(ServerMessage, Vec<u8>)> {
    vec![
      (
        ServerMessage::Announce {
          route: vec![uuid!["732037af-d384-4d93-ab4e-ebaf64de871b"].into()],
          clients: HashMap::from([(
            uuid!["27293ea0-23c5-49e3-97ba-9d9337c1f414"].into(),
            "hardcoded".into(),
          )]),
        },
        vec![
          0, 1, 16, 115, 32, 55, 175, 211, 132, 77, 147, 171, 78, 235, 175, 100, 222, 135, 27, 1,
          16, 39, 41, 62, 160, 35, 197, 73, 227, 151, 186, 157, 147, 55, 193, 244, 20, 9, 104, 97,
          114, 100, 99, 111, 100, 101, 100,
        ],
      ),
      (
        ServerMessage::Message(FullyQualifiedMessage {
          src: uuid!["50064dda-865d-4070-a843-aaca292cb85e"].into(),
          srcsrv: uuid!["95bf0cec-bcf2-4a81-b61a-53ddb36f145d"].into(),
          dsts: vec![
            (
              uuid!["a77f772f-700a-4074-9b84-e264050dab59"].into(),
              uuid!["2f06fd7a-8e7b-4686-9f7d-66a8e4e89152"].into(),
            ),
            (
              uuid!["5b826b4d-f330-4b5f-83ae-c6fe05b7f760"].into(),
              uuid!["6d1a83bf-c901-416c-8ab3-12409e090a0f"].into(),
            ),
          ],
          content: "Yes!".into(),
        }),
        vec![
          1, 16, 80, 6, 77, 218, 134, 93, 64, 112, 168, 67, 170, 202, 41, 44, 184, 94, 16, 149,
          191, 12, 236, 188, 242, 74, 129, 182, 26, 83, 221, 179, 111, 20, 93, 2, 16, 167, 127,
          119, 47, 112, 10, 64, 116, 155, 132, 226, 100, 5, 13, 171, 89, 16, 47, 6, 253, 122, 142,
          123, 70, 134, 159, 125, 102, 168, 228, 232, 145, 82, 16, 91, 130, 107, 77, 243, 48, 75,
          95, 131, 174, 198, 254, 5, 183, 247, 96, 16, 109, 26, 131, 191, 201, 1, 65, 108, 138,
          179, 18, 64, 158, 9, 10, 15, 4, 89, 101, 115, 33,
        ],
      ),
    ]
  }

  fn auth_hardcoded() -> Vec<(AuthMessage, Vec<u8>)> {
    vec![
      (
        AuthMessage::Auth {
          response: [
            199, 15, 141, 177, 121, 9, 136, 28, 127, 31, 230, 116, 79, 121, 111, 122,
          ],
        },
        vec![
          2, 199, 15, 141, 177, 121, 9, 136, 28, 127, 31, 230, 116, 79, 121, 111, 122,
        ],
      ),
      (
        AuthMessage::Hello {
          user: uuid!["45095b4e-549d-4fd9-b4d0-9aa4111c6324"].into(),
          nonce: [160, 172, 206, 207, 7, 198, 123, 142],
        },
        vec![
          0, 16, 69, 9, 91, 78, 84, 157, 79, 217, 180, 208, 154, 164, 17, 28, 99, 36, 160, 172,
          206, 207, 7, 198, 123, 142,
        ],
      ),
      (
        AuthMessage::Nonce {
          server: uuid!["2a1e715b-5a5e-406b-9046-7be132a8df27"].into(),
          nonce: [185, 213, 83, 150, 85, 248, 241, 110],
        },
        vec![
          1, 16, 42, 30, 113, 91, 90, 94, 64, 107, 144, 70, 123, 225, 50, 168, 223, 39, 185, 213,
          83, 150, 85, 248, 241, 110,
        ],
      ),
    ]
  }

  fn client_hardcoded() -> Vec<(ClientMessage, Vec<u8>)> {
    vec![
      (
        ClientMessage::Text {
          dest: uuid!["732037af-d384-4d93-ab4e-ebaf64de871b"].into(),
          content: "P2s6ERp2".into(),
        },
        vec![
          0, 16, 115, 32, 55, 175, 211, 132, 77, 147, 171, 78, 235, 175, 100, 222, 135, 27, 8, 80,
          50, 115, 54, 69, 82, 112, 50,
        ],
      ),
      (
        ClientMessage::MText {
          dest: vec![
            uuid!["c770520b-cb20-4f48-8a52-91d4c6fc0822"].into(),
            uuid!["27293ea0-23c5-49e3-97ba-9d9337c1f414"].into(),
            uuid!["13ca9cc9-82df-46e4-8a10-1e32379280f0"].into(),
            uuid!["30be499a-4d4e-456a-9310-404679c203c2"].into(),
          ],
          content: "g1tL1R58x5C05jc".into(),
        },
        vec![
          1, 4, 16, 199, 112, 82, 11, 203, 32, 79, 72, 138, 82, 145, 212, 198, 252, 8, 34, 16, 39,
          41, 62, 160, 35, 197, 73, 227, 151, 186, 157, 147, 55, 193, 244, 20, 16, 19, 202, 156,
          201, 130, 223, 70, 228, 138, 16, 30, 50, 55, 146, 128, 240, 16, 48, 190, 73, 154, 77, 78,
          69, 106, 147, 16, 64, 70, 121, 194, 3, 194, 15, 103, 49, 116, 76, 49, 82, 53, 56, 120,
          53, 67, 48, 53, 106, 99,
        ],
      ),
    ]
  }

  fn round_trip<T, ENC, DEC>(e: ENC, d: DEC, clear: &T, encoded: &[u8])
  where
    T: Eq + std::fmt::Debug,
    ENC: FnOnce(&mut Cursor<Vec<u8>>, &T) -> std::io::Result<()>,
    DEC: FnOnce(&mut Cursor<Vec<u8>>) -> anyhow::Result<T>,
  {
    println!("test {:?} <-> {:?}", clear, encoded);
    let mut wr = Cursor::new(Vec::new());
    e(&mut wr, clear).unwrap();
    let buf = wr.into_inner();
    assert_eq!(buf, encoded);

    let mut cursor = Cursor::new(buf);
    let decoded = d(&mut cursor).unwrap();
    assert_eq!(&decoded, clear);
  }

  #[test]
  fn u128() {
    let samples: [(u128, &[u8]); 10] = [
      (1, &[1]),
      (0xff, &[251, 255, 0]),
      (0x1234, &[251, 52, 18]),
      (0x123456, &[252, 86, 52, 18, 0]),
      (0x12345678, &[252, 120, 86, 52, 18]),
      (0x123456789a, &[253, 154, 120, 86, 52, 18, 0, 0, 0]),
      (0x123456789abc, &[253, 188, 154, 120, 86, 52, 18, 0, 0]),
      (0x123456789abcde, &[253, 222, 188, 154, 120, 86, 52, 18, 0]),
      (
        0x123456789abcdef0,
        &[253, 240, 222, 188, 154, 120, 86, 52, 18],
      ),
      (
        0xffffffffffffffff,
        &[253, 255, 255, 255, 255, 255, 255, 255, 255],
      ),
    ];
    for (raw, encoded) in samples {
      round_trip(
        |w, x: &u128| encode::u128(w, *x),
        decode::u128,
        &raw,
        encoded,
      );
    }
  }

  #[test]
  fn serverid_encode() {
    let source = ServerId(uuid!["a3b674a2-b950-4e44-b32b-a29345e38e36"]);
    let mut wr = Cursor::new(Vec::new());
    encode::serverid(&mut wr, &source).unwrap();
    assert_eq!(
      wr.into_inner(),
      &[16, 163, 182, 116, 162, 185, 80, 78, 68, 179, 43, 162, 147, 69, 227, 142, 54]
    )
  }

  #[test]
  fn serverid_decode() {
    let expected = ServerId(uuid!["a3b674a2-b950-4e44-b32b-a29345e38e36"]);
    let mut rd = Cursor::new([
      16, 163, 182, 116, 162, 185, 80, 78, 68, 179, 43, 162, 147, 69, 227, 142, 54,
    ]);
    let decoded = decode::serverid(&mut rd).unwrap();
    assert_eq!(decoded, expected);
  }

  #[test]
  fn server_round_trip() {
    for msg in servermessages() {
      let mut wr = Cursor::new(Vec::new());
      encode::server(&mut wr, &msg).unwrap();
      let mut cursor = Cursor::new(wr.into_inner());
      let decoded = decode::server(&mut cursor).unwrap();
      assert_eq!(decoded, msg);
    }
  }

  #[test]
  fn server_encode() {
    for (msg, expected) in server_hardcoded() {
      let mut wr = Cursor::new(Vec::new());
      encode::server(&mut wr, &msg).unwrap();
      assert_eq!(wr.into_inner(), expected)
    }
  }

  #[test]
  fn server_decode() {
    for (expected, buf) in server_hardcoded() {
      let mut rd = Cursor::new(buf);
      let decoded = decode::server(&mut rd).unwrap();
      assert_eq!(decoded, expected);
    }
  }

  #[test]
  fn auth_encode() {
    for (msg, expected) in auth_hardcoded() {
      let mut wr = Cursor::new(Vec::new());
      encode::auth(&mut wr, &msg).unwrap();
      assert_eq!(wr.into_inner(), expected)
    }
  }

  #[test]
  fn auth_decode() {
    for (expected, buf) in auth_hardcoded() {
      let mut rd = Cursor::new(buf);
      let decoded = decode::auth(&mut rd).unwrap();
      assert_eq!(decoded, expected);
    }
  }

  #[test]
  fn client_encode() {
    for (msg, expected) in client_hardcoded() {
      let mut wr = Cursor::new(Vec::new());
      encode::client(&mut wr, &msg).unwrap();
      assert_eq!(wr.into_inner(), expected)
    }
  }

  #[test]
  fn client_decode() {
    for (expected, buf) in client_hardcoded() {
      let mut rd = Cursor::new(buf);
      let decoded = decode::client(&mut rd).unwrap();
      assert_eq!(decoded, expected);
    }
  }
  #[test]
  fn unicode() {
    let msg = ClientMessage::Text {
      dest: ClientId::from(126u128),
      content: "😘😙😚".to_string(),
    };
    let mut wr = Cursor::new(Vec::new());
    encode::client(&mut wr, &msg).unwrap();
    let mut cursor = Cursor::new(wr.into_inner());
    let decoded = decode::client(&mut cursor).unwrap();
    assert_eq!(decoded, msg);
  }

  #[test]
  fn string_encode() {
    let src = "Hello World ;)".to_string();
    let mut wr = Cursor::new(Vec::new());
    encode::string(&mut wr, &src).unwrap();
    assert_eq!(
      wr.into_inner(),
      [14, 72, 101, 108, 108, 111, 32, 87, 111, 114, 108, 100, 32, 59, 41]
    );
  }

  #[test]
  fn client_query_register() {
    let query = ClientQuery::Register("Bob".into());
    round_trip(
      encode::client_query,
      decode::client_query,
      &query,
      &[0, 3, 66, 111, 98],
    );
  }

  #[test]
  fn client_query_poll() {
    let query = ClientQuery::Poll;
    round_trip(encode::client_query, decode::client_query, &query, &[2]);
  }

  #[test]
  fn client_query_list_users() {
    let query = ClientQuery::ListUsers;
    round_trip(encode::client_query, decode::client_query, &query, &[3]);
  }

  #[test]
  fn string_decode() {
    let mut cursor = Cursor::new([
      14, 72, 101, 108, 108, 111, 32, 87, 111, 114, 108, 100, 32, 59, 41,
    ]);
    let decoded = decode::string(&mut cursor).unwrap();
    assert_eq!(decoded, "Hello World ;)");
  }

  #[test]
  fn sequence() {
    let src = Sequence {
      seqid: 12,
      src: uuid!["77ff529e-75bd-4832-bf0c-6db339022924"].into(),
      content: "Hello".to_string(),
    };
    let encoded = &[
      12, 16, 119, 255, 82, 158, 117, 189, 72, 50, 191, 12, 109, 179, 57, 2, 41, 36, 5, 72, 101, 108, 108, 111,
    ];
    round_trip::<Sequence<String>, _, _>(
      |w, seq| encode::sequence(w, seq, |w2, st| encode::string(w2, st.as_str())),
      |rd| decode::sequence(rd, decode::string),
      &src,
      encoded,
    );
  }
}
