use crate::protocol::payload::Version;

use sha2::{Digest, Sha256};
use tokio::{io::AsyncReadExt, net::TcpStream};

use std::{io, io::Write};

const MAGIC: [u8; 4] = [0xfa, 0x1a, 0xf9, 0xbf];

const VERSION_COMMAND: [u8; 12] = *b"version\0\0\0\0\0";
const VERACK_COMMAND: [u8; 12] = *b"verack\0\0\0\0\0\0";

#[derive(Debug, Default)]
pub struct MessageHeader {
    magic: [u8; 4],
    command: [u8; 12],
    body_length: u32,
    checksum: u32,
}

impl MessageHeader {
    pub fn new(command: [u8; 12], body: &[u8]) -> Self {
        MessageHeader {
            magic: MAGIC,
            command,
            body_length: body.len() as u32,
            checksum: checksum(body),
        }
    }

    pub async fn write_to_stream(&self, stream: &mut TcpStream) -> io::Result<()> {
        let mut buffer = vec![];

        buffer.write_all(&self.magic)?;
        buffer.write_all(&self.command)?;
        buffer.write_all(&self.body_length.to_le_bytes())?;
        buffer.write_all(&self.checksum.to_le_bytes())?;

        tokio::io::AsyncWriteExt::write_all(stream, &buffer).await?;

        Ok(())
    }

    pub async fn read_from_stream(stream: &mut TcpStream) -> io::Result<Self> {
        let mut header: MessageHeader = Default::default();

        stream.read_exact(&mut header.magic).await?;
        stream.read_exact(&mut header.command).await?;
        header.body_length = stream.read_u32_le().await?;
        header.checksum = stream.read_u32_le().await?;

        Ok(header)
    }
}

pub enum Message {
    Version(Version),
    Verack,
}

impl Message {
    pub async fn write_to_stream(&self, stream: &mut TcpStream) -> io::Result<()> {
        // Buffer for the message payload.
        let mut buffer = vec![];

        let header = match self {
            Self::Version(version) => {
                version.encode(&mut buffer)?;
                MessageHeader::new(VERSION_COMMAND, &buffer)
            }
            Self::Verack => MessageHeader::new(VERACK_COMMAND, &buffer),
        };

        header.write_to_stream(stream).await?;
        tokio::io::AsyncWriteExt::write_all(stream, &buffer).await?;

        Ok(())
    }

    pub async fn read_from_stream(stream: &mut TcpStream) -> io::Result<Self> {
        let header = MessageHeader::read_from_stream(stream).await?;

        let mut bytes = vec![0u8; header.body_length as usize];
        stream
            .read_exact(&mut bytes[..header.body_length as usize])
            .await?;

        let message = match header.command {
            VERSION_COMMAND => Self::Version(Version::decode(&bytes)?),
            VERACK_COMMAND => Self::Verack,
            _ => unimplemented!(),
        };

        Ok(message)
    }
}

fn checksum(bytes: &[u8]) -> u32 {
    let sha2 = Sha256::digest(bytes);
    let sha2d = Sha256::digest(&sha2);

    let mut checksum = [0u8; 4];
    checksum[0..4].copy_from_slice(&sha2d[0..4]);

    u32::from_le_bytes(checksum)
}
