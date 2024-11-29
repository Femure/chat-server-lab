use std::{collections::HashMap, io::Read};

use byteorder::{LittleEndian, ReadBytesExt};
use uuid::Uuid;

use crate::messages::{
  AuthMessage, ClientId, ClientMessage, ClientPollReply, ClientQuery, ClientReply, Sequence,
  ServerId, ServerMessage,
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
  rd.read_u8();
  rd.read_exact(buf)
}

// hint: reuse uuid
pub fn clientid<R: Read>(rd: &mut R) -> anyhow::Result<ClientId> {
  todo!()
}

// hint: reuse uuid
pub fn serverid<R: Read>(rd: &mut R) -> anyhow::Result<ServerId> {
  todo!()
}

pub fn string<R: Read>(rd: &mut R) -> anyhow::Result<String> {
  todo!()
}

pub fn auth<R: Read>(rd: &mut R) -> anyhow::Result<AuthMessage> {
  todo!()
}

pub fn client<R: Read>(rd: &mut R) -> anyhow::Result<ClientMessage> {
  todo!()
}

pub fn client_replies<R: Read>(rd: &mut R) -> anyhow::Result<Vec<ClientReply>> {
  todo!()
}

pub fn client_poll_reply<R: Read>(rd: &mut R) -> anyhow::Result<ClientPollReply> {
  todo!()
}

pub fn server<R: Read>(rd: &mut R) -> anyhow::Result<ServerMessage> {
  todo!()
}

pub fn userlist<R: Read>(rd: &mut R) -> anyhow::Result<HashMap<ClientId, String>> {
  todo!()
}

pub fn client_query<R: Read>(rd: &mut R) -> anyhow::Result<ClientQuery> {
  todo!()
}

pub fn sequence<X, R: Read, DEC>(rd: &mut R, d: DEC) -> anyhow::Result<Sequence<X>>
where
  DEC: FnOnce(&mut R) -> anyhow::Result<X>,
{
  todo!()
}
