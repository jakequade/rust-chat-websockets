use std::error::Error;
use std::io;
use std::io::{Read, Write};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

const PAYLOAD_LEN_U16: u8 = 126;
const PAYLOAD_LEN_U64: u8 = 127;

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum OpCode {
    TextFrame = 1,
    BinaryFrame = 2,
    ConnectionClose = 8,
    Ping = 9,
    Pong = 0xA,
}

impl OpCode {
    fn from(op: u8) -> Option<OpCode> {
        match op {
            1 => Some(OpCode::TextFrame),
            2 => Some(OpCode::BinaryFrame),
            8 => Some(OpCode::ConnectionClose),
            9 => Some(OpCode::Ping),
            0xA => Some(OpCode::Pong),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct WebSocketFrameHeader {
    fin: bool,
    rsv1: bool,
    rsv2: bool,
    rsv3: bool,
    masked: bool,
    opcode: OpCode,
    payload_length: u8,
}

impl WebSocketFrameHeader {
    fn new_header(len: usize, opcode: OpCode) -> WebSocketFrameHeader {
        WebSocketFrameHeader {
            fin: true,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            masked: false,
            payload_length: Self::determine_length(len),
            opcode,
        }
    }

    fn determine_length(len: usize) -> u8 {
        if len < (PAYLOAD_LEN_U16 as usize) {
            len as u8
        } else if len < (u16::MAX as usize) {
            PAYLOAD_LEN_U16
        } else {
            PAYLOAD_LEN_U64
        }
    }
}

impl<'a> From<&'a str> for WebSocketFrame {
    fn from(payload: &str) -> WebSocketFrame {
        WebSocketFrame {
            header: WebSocketFrameHeader::new_header(payload.len(), OpCode::TextFrame),
            payload: Vec::from(payload),
            mask: None,
        }
    }
}

#[derive(Debug)]
pub struct WebSocketFrame {
    header: WebSocketFrameHeader,
    mask: Option<[u8; 4]>,
    pub payload: Vec<u8>,
}

impl WebSocketFrame {
    pub fn read<R: Read>(input: &mut R) -> io::Result<WebSocketFrame> {
        let buf = input.read_u16::<BigEndian>()?;
        let header = Self::parse_header(buf).unwrap();

        let len = Self::read_length(header.payload_length, input)?;
        let mask_key = if header.masked {
            let mask = Self::read_mask(input)?;
            Some(mask)
        } else {
            None
        };

        let mut payload = Self::read_payload(len, input)?;

        if let Some(mask) = mask_key {
            Self::apply_mask(mask, &mut payload);
        }

        Ok(WebSocketFrame {
            header,
            payload,
            mask: mask_key,
        })
    }

    pub fn write<W: Write>(&self, output: &mut W) -> io::Result<()> {
        let hdr = Self::serialize_header(&self.header);

        output.write_u16::<BigEndian>(hdr)?;

        match self.header.payload_length {
            PAYLOAD_LEN_U16 => output
                .write_u16::<BigEndian>(self.payload.len() as u16)
                .unwrap(),
            PAYLOAD_LEN_U64 => output
                .write_u64::<BigEndian>(self.payload.len() as u64)
                .unwrap(),
            _ => {}
        };

        Ok(())
    }

    pub fn get_opcode(&self) -> OpCode {
        self.header.opcode.clone()
    }

    fn parse_header(buf: u16) -> Result<WebSocketFrameHeader, String> {
        let opcode_num = ((buf >> 8) as u8) & 0x0F;
        let opcode = OpCode::from(opcode_num);

        if let Some(opcode) = opcode {
            Ok(WebSocketFrameHeader {
                fin: (buf >> 8) & 0x80 == 0x80,
                rsv1: (buf >> 8) & 0x40 == 0x40,
                rsv2: (buf >> 8) & 0x20 == 0x20,
                rsv3: (buf >> 8) & 0x10 == 0x10,
                opcode,
                masked: buf & 0x80 == 0x80,
                payload_length: (buf as u8) & 0x7F,
            })
        } else {
            Err(format!("Invalid Opcode: {}", opcode_num))
        }
    }

    fn serialize_header(hdr: &WebSocketFrameHeader) -> u16 {
        let b1 = ((hdr.fin as u8) << 7)
            | ((hdr.rsv1 as u8) << 6)
            | ((hdr.rsv2 as u8) << 5)
            | ((hdr.rsv3 as u8) << 4)
            | ((hdr.opcode as u8) & 0x0F);

        let b2 = ((hdr.masked as u8) << 7) | ((hdr.payload_length as u8) & 0x7F);

        ((b1 as u16) << 8) | (b2 as u16)
    }

    fn apply_mask(mask: [u8; 4], bytes: &mut Vec<u8>) {
        for (idx, c) in bytes.iter_mut().enumerate() {
            // XOR the payload byte by whatever value the mask is
            *c = *c ^ mask[idx % 4];
        }
    }

    fn read_mask<R: Read>(input: &mut R) -> io::Result<[u8; 4]> {
        let mut buf = [0; 4];
        input.read(&mut buf)?;

        Ok(buf)
    }

    fn read_length<R: Read>(payload_len: u8, input: &mut R) -> io::Result<usize> {
        match payload_len {
            PAYLOAD_LEN_U64 => input
                .read_u64::<BigEndian>()
                .map(|v| v as usize)
                .map_err(From::from),
            PAYLOAD_LEN_U16 => input
                .read_u16::<BigEndian>()
                .map(|v| v as usize)
                .map_err(From::from),
            _ => Ok(payload_len as usize), // payload less than 127
        }
    }

    fn read_payload<R: Read>(payload_len: usize, input: &mut R) -> io::Result<Vec<u8>> {
        let mut payload: Vec<u8> = Vec::with_capacity(payload_len);
        payload.extend(std::iter::repeat(0).take(payload_len));

        input.read(&mut payload)?;
        Ok(payload)
    }
}
