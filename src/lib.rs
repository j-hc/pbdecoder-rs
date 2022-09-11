use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};

type VarInt = u64;

mod ptypes {
    pub const VARINT: u64 = 0;
    pub const FIXED64: u64 = 1;
    pub const STRING: u64 = 2;
    pub const FIXED32: u64 = 5;
}

#[derive(Clone, Debug)]
#[cfg_attr(
    feature = "serialize",
    derive(Serialize, Deserialize),
    serde(rename_all = "lowercase", tag = "type", content = "value")
)]
pub enum ProtoValue {
    Fixed32(u32),
    Fixed64(u64),
    String(String),
    VarInt(VarInt),
    Parts(Vec<ProtoPart>),
}

#[derive(Clone, Debug)]
#[cfg_attr(
    feature = "serialize",
    derive(Serialize, Deserialize),
    serde(rename_all = "lowercase")
)]
pub struct ProtoPart {
    pub index: u64,
    #[cfg_attr(feature = "serialize", serde(skip))]
    pub offset: usize,
    #[cfg_attr(feature = "serialize", serde(flatten))]
    pub value: ProtoValue,
}

#[derive(Debug)]
enum DecodeErr {
    IOErr(io::Error),
    UnknownType,
}

impl From<io::Error> for DecodeErr {
    fn from(e: io::Error) -> Self {
        Self::IOErr(e)
    }
}

impl ProtoPart {
    fn new(index: u64, offset: usize, value: ProtoValue) -> Self {
        Self {
            index,
            offset,
            value,
        }
    }

    // this probably doesnt work
    pub fn write(&self, new_val: ProtoValue, buf: &mut [u8]) -> io::Result<()> {
        if std::mem::discriminant(&self.value) != std::mem::discriminant(&new_val) {
            panic!("non-matching value types");
        }
        let mut buf = &mut buf[self.offset..];
        let r = match new_val {
            ProtoValue::Fixed32(v) => buf.write_u32::<BigEndian>(v),
            ProtoValue::Fixed64(v) | ProtoValue::VarInt(v) => buf.write_u64::<BigEndian>(v),
            ProtoValue::String(v) => (&mut buf[..v.len()]).write_all(v.as_bytes()),
            _ => panic!(),
        };

        r
    }
}

struct ProtoBufReader<'a> {
    cur: Cursor<&'a [u8]>,
    checkpoint: u64,
}
impl<'a> ProtoBufReader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self {
            cur: Cursor::new(buf),
            checkpoint: 0,
        }
    }

    fn read_varint(&mut self) -> io::Result<VarInt> {
        let mut res: u64 = 0;
        let mut shift: u32 = 0;

        loop {
            let byte = self.cur.read_u8()?;
            let multiplier = 2_u64.pow(shift);
            let byte_value = (byte as u64 & 0x7f) * multiplier;
            shift += 7;
            res += byte_value;
            if byte < 0x80 {
                break;
            }
        }
        Ok(res)
    }

    fn skip_grpc_header(&mut self) {
        let current_offset = self.cur.position();
        if matches!(self.peek(), Some(0)) {
            self.seek(1).unwrap();
            let length = self.cur.read_i32::<BigEndian>().unwrap();

            if length as usize > self.remaining_bytes().len() {
                self.cur.set_position(current_offset);
            }
        }
    }

    fn pos(&self) -> usize {
        self.cur.position() as usize
    }

    fn checkpoint(&mut self) {
        self.checkpoint = self.cur.position()
    }

    fn reset_checkpoint(&mut self) {
        self.cur.set_position(self.checkpoint)
    }

    fn remaining_bytes(&self) -> &'a [u8] {
        &self.cur.get_ref()[self.cur.position() as usize..]
    }

    fn peek(&self) -> Option<u8> {
        self.remaining_bytes().first().copied()
    }

    fn seek(&mut self, offset: i64) -> io::Result<u64> {
        self.cur.seek(SeekFrom::Current(offset))
    }
}

/// decodes the raw protobuf binary and returns with the leftover
pub fn decode_proto(buffer: &[u8]) -> (Vec<ProtoPart>, &[u8]) {
    let mut reader = ProtoBufReader::new(buffer);
    reader.skip_grpc_header();
    let mut parts = Vec::new();

    while !reader.remaining_bytes().is_empty() {
        reader.checkpoint();
        match decode_part(&mut reader) {
            Ok(part) => parts.push(part),
            Err(_) => {
                reader.reset_checkpoint();
                break;
            }
        }
    }
    (parts, reader.remaining_bytes())
}

fn decode_part(reader: &mut ProtoBufReader) -> Result<ProtoPart, DecodeErr> {
    let index_type = reader.read_varint()?;
    let ptype = index_type & 0x7;
    let index = index_type >> 0x3;
    Ok(match ptype {
        ptypes::VARINT => {
            let offset = reader.pos();
            let value = reader.read_varint()?;
            ProtoPart::new(index, offset, ProtoValue::VarInt(value))
        }
        ptypes::STRING => {
            let length = reader.read_varint()?;
            let mut buf = vec![0; length as usize];
            let offset = reader.pos();
            reader.cur.read_exact(&mut buf)?;

            let decoded = decode_proto(&buf);
            if !buf.is_empty() && decoded.1.is_empty() {
                ProtoPart::new(index, offset, ProtoValue::Parts(decoded.0))
            } else {
                let s = String::from_utf8_lossy(&buf).to_string();
                ProtoPart::new(index, offset, ProtoValue::String(s))
            }
        }
        ptypes::FIXED32 => {
            let offset = reader.pos();
            let value = reader.cur.read_u32::<BigEndian>()?;
            ProtoPart::new(index, offset, ProtoValue::Fixed32(value))
        }
        ptypes::FIXED64 => {
            let offset = reader.pos();
            let value = reader.cur.read_u64::<BigEndian>()?;
            ProtoPart::new(index, offset, ProtoValue::Fixed64(value))
        }
        _ => return Err(DecodeErr::UnknownType),
    })
}
