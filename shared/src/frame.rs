use bytes::{Buf, BufMut, BytesMut};
use std::{io, marker::PhantomData};
use tokio_util::codec::{Decoder, Encoder};

pub trait Opcode: Copy + Sized {
    fn from_raw(raw: u16) -> Result<Self, u16>;
    fn into_raw(self) -> u16;
}

#[derive(Debug, Clone)]
pub struct MessageFrame<O: Opcode> {
    pub version: u8,
    pub flags: u8,
    pub opcode: O,
    pub payload: Vec<u8>,
}

impl<O: Opcode> MessageFrame<O> {
    pub fn new(opcode: O, payload: Vec<u8>) -> Self {
        Self {
            version: 0,
            flags: 0,
            opcode,
            payload,
        }
    }
}

pub struct MsgCodec<O: Opcode> {
    _marker: PhantomData<O>,
}

impl<O: Opcode> Default for MsgCodec<O> {
    fn default() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<O: Opcode> Decoder for MsgCodec<O> {
    type Item = MessageFrame<O>;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        const HEADER_LEN: usize = 8;
        if src.len() < HEADER_LEN {
            return Ok(None);
        }
        let mut buf = &src[..HEADER_LEN];
        let version = buf.get_u8();
        let flags = buf.get_u8();
        let opcode_raw = buf.get_u16_le();
        let len = buf.get_u32_le() as usize;

        if src.len() < HEADER_LEN + len {
            src.reserve(HEADER_LEN + len - src.len());
            return Ok(None);
        }

        src.advance(HEADER_LEN);
        let payload = src.split_to(len).to_vec();

        let opcode = O::from_raw(opcode_raw).map_err(|raw| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unknown opcode {raw:#06x}"),
            )
        })?;

        Ok(Some(MessageFrame {
            version,
            flags,
            opcode,
            payload,
        }))
    }
}

impl<O: Opcode> Encoder<MessageFrame<O>> for MsgCodec<O> {
    type Error = io::Error;

    fn encode(&mut self, frame: MessageFrame<O>, dst: &mut BytesMut) -> Result<(), Self::Error> {
        const HEADER_LEN: usize = 8;
        dst.reserve(HEADER_LEN + frame.payload.len());
        dst.put_u8(frame.version);
        dst.put_u8(frame.flags);
        dst.put_u16_le(frame.opcode.into_raw());
        dst.put_u32_le(frame.payload.len() as u32);
        dst.extend_from_slice(&frame.payload);
        Ok(())
    }
}
