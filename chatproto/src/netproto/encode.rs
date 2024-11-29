use std::{collections::HashMap, io::Write};

use byteorder::{LittleEndian, WriteBytesExt};
use uuid::Uuid;

use crate::messages::{
  AuthMessage, ClientError, ClientId, ClientMessage, ClientPollReply, ClientQuery, ClientReply, Sequence, ServerId, ServerMessage, DelayedError
};

// look at the README.md for guidance on writing this function
// this function is used to encode all the "sizes" values that will appear after that
pub fn u128<W>(w: &mut W, m: u128) -> std::io::Result<()>
where
  W: Write,
{
  if m < 251 {
    w.write_u8(m as u8)
  } else if m < (1 << 16) {
    w.write_u8(251)?;
    w.write_u16::<LittleEndian>(m as u16)
  } else if m < (1 << 32) {
    w.write_u8(252)?;
    w.write_u32::<LittleEndian>(m as u32)
  } else if m < (1 << 64) {
    w.write_u8(253)?;
    w.write_u64::<LittleEndian>(m as u64)
  } else {
    w.write_u8(254)?;
    w.write_u128::<LittleEndian>(m)
  }
}

/* UUIDs are 128bit values, but in the situation they are represented as [u8; 16]
  don't forget that arrays are encoded with their sizes first, and then their content
*/
fn uuid<W>(w: &mut W, m: &Uuid) -> std::io::Result<()>
where
  W: Write,
{
  let _ = w.write_u8(16);
  w.write_all(m.as_bytes())
}

// reuse uuid
pub fn clientid<W>(w: &mut W, m: &ClientId) -> std::io::Result<()>
where
  W: Write,
{
  uuid(w, &m.0)
}

// reuse uuid
pub fn serverid<W>(w: &mut W, m: &ServerId) -> std::io::Result<()>
where
  W: Write,
{
  uuid(w, &m.0)
}

// strings are encoded as the underlying bytes array
// so
//  1/ get the underlying bytes
//  2/ write the size (using u128)
//  3/ write the array
pub fn string<W>(w: &mut W, m: &str) -> std::io::Result<()>
where
  W: Write,
{
  let bytes = m.as_bytes();
  u128(w, bytes.len() as u128)?;
  w.write_all(bytes)
}

/* The following is VERY mechanical, and should be easy once the general principle is understood

* Structs

   Structs are encoded by encoding all fields, by order of declaration. For example:

   struct Test {
     a: u32,
     b: [u8; 3],
   }

   Test {a: 5, b: [1, 2, 3]} -> [5, 3, 1, 2, 3]  // the first '3' is the array length
   Test {a: 42, b: [5, 6, 7]} -> [42, 3, 5, 6, 7]

* Enums

   Enums are encoded by first writing a tag, corresponding to the variant index, and the the content of the variant.
   For example, if we have:

   enum Test { A, B(u32), C(u32, u32) };

   Test::A is encoded as [0]
   Test::B(8) is encoded as [1, 8]
   Test::C(3, 17) is encoded as [2, 3, 17]

 */

pub fn auth<W>(w: &mut W, m: &AuthMessage) -> std::io::Result<()>
where
  W: Write,
{
  match m {
    AuthMessage::Hello { user, nonce } => {
      w.write_u8(0)?;
      clientid(w, user)?;
      w.write_all(nonce)
    }
    AuthMessage::Nonce { server, nonce } => {
      w.write_u8(1)?;
      serverid(w, server)?;
      w.write_all(nonce)
    }
    AuthMessage::Auth { response } => {
      w.write_u8(2)?;
      w.write_all(response)
    }
  }
}

pub fn server<W>(w: &mut W, m: &ServerMessage) -> std::io::Result<()>
where
  W: Write,
{
  match m {
    ServerMessage::Announce { route, clients } => {
      w.write_u8(0)?;
      u128(w, route.len() as u128)?;
      for r in route {
        serverid(w, r)?;
      }
      u128(w, clients.len() as u128)?;
      for (client, str) in clients {
        clientid(w, client)?;
        string(w, str)?;
      }
    }
    ServerMessage::Message(fully_qualified_message) => {
      w.write_u8(1)?;
      clientid(w, &fully_qualified_message.src)?;
      serverid(w, &fully_qualified_message.srcsrv)?;

      u128(w, fully_qualified_message.dsts.len() as u128)?;
      for (cl, serv) in &fully_qualified_message.dsts {
        clientid(w, cl)?;
        serverid(w, serv)?;
      }

      string(w, &fully_qualified_message.content)?;
    }
  }
  Ok(())
}

pub fn client<W>(w: &mut W, m: &ClientMessage) -> std::io::Result<()>
where
  W: Write,
{
  match m {
    ClientMessage::Text { dest, content } => {
      w.write_u8(0)?;
      clientid(w, dest)?;
      string(w, content)?;
    }
    ClientMessage::MText { dest, content } => {
      w.write_u8(1)?;
      u128(w, dest.len() as u128)?;
      for d in dest {
        clientid(w, d)?;
      }
      string(w, content)?;
    }
  }
  Ok(())
}

pub fn client_replies<W>(w: &mut W, m: &[ClientReply]) -> std::io::Result<()>
where
  W: Write,
{
  for rep in m {
  match rep {
    ClientReply::Delivered => {
      w.write_u8(0)?;
    }
    ClientReply::Error(error) => {
      w.write_u8(1)?; // Variant ID for Error
      match error {
        ClientError::UnknownClient => {
          w.write_u8(0)?;
        }
        ClientError::BoxFull(clientid) => {
          w.write_u8(1)?;
          crate::netproto::encode::clientid(w, clientid)?;
        }
        ClientError::InternalError => {
          w.write_u8(2)?;
        }
      }
  }
    ClientReply::Delayed => {
      w.write_u8(2)?;
    }
    ClientReply::Transfer(server_id, server_message) => {
      w.write_u8(3)?;
      serverid(w, server_id)?;
      server(w, server_message)?;
    }
  }}
  Ok(())
}

pub fn client_poll_reply<W>(w: &mut W, m: &ClientPollReply) -> std::io::Result<()>
where
  W: Write,
{
  match m {
    ClientPollReply::Message { src, content } => {
      w.write_u8(0)?;
      clientid(w, src)?;
      string(w, content)?;
    }
    ClientPollReply::DelayedError(delayed_error) => {
      w.write_u8(1)?;
      match delayed_error {
        DelayedError::UnknownRecipient(client_id) => clientid(w, client_id)?,
      }
    }
    ClientPollReply::Nothing => {
      w.write_u8(2)?;
    }
  }
  Ok(())
}

// hashmaps are encoded by first writing the size (using u128), then each key and values
pub fn userlist<W>(w: &mut W, m: &HashMap<ClientId, String>) -> std::io::Result<()>
where
  W: Write,
{
  u128(w, m.len() as u128)?;
  for (client, str) in m {
    clientid(w, client)?;
    string(w, str)?;
  }
  Ok(())
}

pub fn client_query<W>(w: &mut W, m: &ClientQuery) -> std::io::Result<()>
where
    W: Write,
{
    match m {
        ClientQuery::Register(name) => {
            w.write_u8(0)?;
            string(w, name)?;
        }
        ClientQuery::Message(client_message) => {
            w.write_u8(1)?;
            client(w, client_message)?;
        }
        ClientQuery::Poll => {
            w.write_u8(2)?;
        }
        ClientQuery::ListUsers => {
            w.write_u8(3)?;
        }
    }

    Ok(())
}


pub fn sequence<X, W, ENC>(w: &mut W, m: &Sequence<X>, f: ENC) -> std::io::Result<()>
where
    W: Write,
    X: serde::Serialize,
    ENC: FnOnce(&mut W, &X) -> std::io::Result<()>,
{
    u128(w, m.seqid)?;
    clientid(w, &m.src)?;
    f(w, &m.content)?;
    Ok(())
}

