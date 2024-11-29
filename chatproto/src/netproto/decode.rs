use std::{collections::HashMap, io::Read};

use anyhow::Ok;
use byteorder::{LittleEndian, ReadBytesExt};
use uuid::Uuid;

use crate::messages::{
  AuthMessage, ClientError, ClientId, ClientMessage, ClientPollReply, ClientQuery, ClientReply, DelayedError, FullyQualifiedMessage, Sequence, ServerId, ServerMessage
};

// look at the README.md for guidance on writing this function
pub fn u128<R: Read>(rd: &mut R) -> anyhow::Result<u128> {
  let prefix = rd.read_u8()?;

  match prefix {
    0..=250 => Ok(prefix as u128),
    251 => {
      let value = rd.read_u16::<LittleEndian>()?;
      Ok(value as u128)
    }
    252 => {
      let value = rd.read_u32::<LittleEndian>()?;
      Ok(value as u128)
    }
    253 => {
      let value = rd.read_u64::<LittleEndian>()?;
      Ok(value as u128)
    }
    254 => {
      let value = rd.read_u128::<LittleEndian>()?;
      Ok(value)
    }
    _ => Err(anyhow::anyhow!("Invalid prefix byte for u128 encoding")),
  }
}

fn uuid<R: Read>(rd: &mut R) -> anyhow::Result<Uuid> {
  if rd.read_u8().unwrap() == 16 {
    let mut buffer = [0; 16];
    rd.read_exact(&mut buffer)?;
    Ok(Uuid::from_bytes(buffer))
  } else {
    Err(anyhow::anyhow!("Invalid prefix byte for u8 encoding"))
  }
}

// hint: reuse uuid
pub fn clientid<R: Read>(rd: &mut R) -> anyhow::Result<ClientId> {
  Ok(ClientId(uuid(rd)?))
}

// hint: reuse uuid
pub fn serverid<R: Read>(rd: &mut R) -> anyhow::Result<ServerId> {
  Ok(ServerId(uuid(rd)?))
}

pub fn string<R: Read>(rd: &mut R) -> anyhow::Result<String> {
  let size = u128(rd)? as usize;
  let mut buf = vec![0u8; size];
  rd.read_exact(&mut buf)?;
  Ok(String::from_utf8(buf)?)
}

pub fn auth<R: Read>(rd: &mut R) -> anyhow::Result<AuthMessage> {
  let variant = rd.read_u8()?;
  match variant {
    0 => {
      let user = clientid(rd)?;
      let mut nonce = [0u8; 8];
      rd.read_exact(&mut nonce)?;
      Ok(AuthMessage::Hello { user, nonce })
    }
    1 => {
      let server = serverid(rd)?;
      let mut nonce = [0u8; 8];
      rd.read_exact(&mut nonce)?;
      Ok(AuthMessage::Nonce { server, nonce })
    }
    2 => {
      let mut response = [0u8; 16];
      rd.read_exact(&mut response)?;
      Ok(AuthMessage::Auth { response })
    }
    _ => Err(anyhow::anyhow!("Invalid AuthMessage")),
  }
}

pub fn server<R: Read>(rd: &mut R) -> anyhow::Result<ServerMessage> {
  let variant = rd.read_u8()?;
  match variant {
    0 => {
      let nb_routes = u128(rd)? as usize;
      let mut route = Vec::new();
      for _ in 0..nb_routes {
        route.push(serverid(rd)?);
      }
      let nb_clients = u128(rd)? as usize;
      let mut clients = HashMap::new();
      for _ in 0..nb_clients {
        clients.insert(clientid(rd)?, string(rd)?);
      }
      Ok(ServerMessage::Announce { route, clients })
    }
    1 => {
      let src = clientid(rd)?;
      let srcsrv = serverid(rd)?;

      let nb_dsts = u128(rd)? as usize;
      let mut dsts = Vec::new();
      for _ in 0..nb_dsts {
        dsts.push((clientid(rd)?, serverid(rd)?));
      }

      let content = string(rd)?;
      Ok(ServerMessage::Message(FullyQualifiedMessage {
        src,
        srcsrv,
        dsts,
        content,
      }))
    }
    _ => Err(anyhow::anyhow!("Invalid ServerMessage")),
  }
}

pub fn client<R: Read>(rd: &mut R) -> anyhow::Result<ClientMessage> {
  let variant = rd.read_u8()?;
  match variant {
    0 => {
      let dest = clientid(rd)?;
      let content = string(rd)?;
      Ok(ClientMessage::Text { dest, content })
    }
    1 => {
      let nb_dest = u128(rd)? as usize;
      let mut dest = Vec::new();
      for _ in 0..nb_dest {
        dest.push(clientid(rd)?);
      }
      let content = string(rd)?;
      Ok(ClientMessage::MText { dest, content })
    }
    _ => Err(anyhow::anyhow!("Invalid ClientMessage")),
  }
}

pub fn client_replies<R: Read>(rd: &mut R) -> anyhow::Result<Vec<ClientReply>> {
  let nb_replies = u128(rd)? as usize;
  let mut replies = Vec::with_capacity(nb_replies);

  for _ in 0..nb_replies {
      let variant = rd.read_u8()?;
      let reply = match variant {
          0 => ClientReply::Delivered,
          1 => {
              let error_variant = rd.read_u8()?;
              let error = match error_variant {
                  0 => ClientError::UnknownClient,
                  1 => ClientError::BoxFull(clientid(rd)?),
                  2 => ClientError::InternalError,
                  _ => return Err(anyhow::anyhow!("Invalid ClientError variant")),
              };
              ClientReply::Error(error)
          }
          2 => ClientReply::Delayed,
          3 => {
              let server_id = serverid(rd)?;
              let server_message = server(rd)?;
              ClientReply::Transfer(server_id, server_message)
          }
          _ => return Err(anyhow::anyhow!("Invalid ClientReply variant")),
      };
      replies.push(reply);
  }

  Ok(replies)
}


pub fn client_poll_reply<R: Read>(rd: &mut R) -> anyhow::Result<ClientPollReply> {
  let variant = rd.read_u8()?;
  match variant {
    0 => {
      let src = clientid(rd)?;
      let content = string(rd)?;
      Ok(ClientPollReply::Message { src, content })
    }
    1 => {
      let delayed_error = clientid(rd)?;
      Ok(ClientPollReply::DelayedError(
        DelayedError::UnknownRecipient(delayed_error),
      ))
    }
    2 => Ok(ClientPollReply::Nothing),
    _ => Err(anyhow::anyhow!("Invalid ClientPollReply")),
  }
}

pub fn userlist<R: Read>(rd: &mut R) -> anyhow::Result<HashMap<ClientId, String>> {
  let nb_users = u128(rd)? as usize;
  let mut users = HashMap::new();
  for _ in 0..nb_users {
    users.insert(clientid(rd)?, string(rd)?);
  }
  Ok(users)
}

pub fn client_query<R: Read>(rd: &mut R) -> anyhow::Result<ClientQuery> {
  let variant = rd.read_u8()?;
  match variant {
      0 => Ok(ClientQuery::Register(string(rd)?)),
      1 => Ok(ClientQuery::Message(client(rd)?)),
      2 => Ok(ClientQuery::Poll),
      3 => Ok(ClientQuery::ListUsers),
      _ => Err(anyhow::anyhow!("Invalid ClientQuery variant")),
  }
}


pub fn sequence<R, X, DEC>(rd: &mut R, d: DEC) -> anyhow::Result<Sequence<X>>
where
    R: Read,
    DEC: FnOnce(&mut R) -> anyhow::Result<X>,
{
    let seqid = u128(rd)?;
    let src = clientid(rd)?;
    let content = d(rd)?;
    Ok(Sequence { seqid, src, content })
}
