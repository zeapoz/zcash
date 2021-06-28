//! Version payload types.

use crate::protocol::payload::{
    addr::NetworkAddr, codec::Codec, read_n_bytes, read_timestamp, Nonce, ProtocolVersion, VarStr,
};

use chrono::{DateTime, Utc};

use std::{
    io::{self, Cursor, Write},
    net::SocketAddr,
};

#[derive(Debug, PartialEq, Clone)]
pub struct Version {
    version: ProtocolVersion,
    services: u64,
    timestamp: DateTime<Utc>,
    addr_recv: NetworkAddr,
    pub addr_from: NetworkAddr,
    pub nonce: Nonce,
    user_agent: VarStr,
    start_height: u32,
    relay: bool,
}

impl Version {
    /// Constructs a [Version], where `addr_recv` is the remote ZCashd/Zebra Node address and
    /// `addr_from` is our local peer node address.
    pub fn new(addr_recv: SocketAddr, addr_from: SocketAddr) -> Self {
        Self {
            version: ProtocolVersion::current(),
            services: 1,
            timestamp: Utc::now(),
            addr_recv: NetworkAddr {
                last_seen: None,
                services: 1,
                addr: addr_recv,
            },
            addr_from: NetworkAddr {
                last_seen: None,
                services: 1,
                addr: addr_from,
            },
            nonce: Nonce::default(),
            user_agent: VarStr(String::from("")),
            start_height: 0,
            relay: false,
        }
    }

    pub fn with_version(mut self, version: u32) -> Self {
        self.version = ProtocolVersion(version);
        self
    }
}

impl Codec for Version {
    fn encode(&self, buffer: &mut Vec<u8>) -> io::Result<()> {
        self.version.encode(buffer)?;
        buffer.write_all(&self.services.to_le_bytes())?;
        buffer.write_all(&self.timestamp.timestamp().to_le_bytes())?;

        self.addr_recv.encode_without_timestamp(buffer)?;
        self.addr_from.encode_without_timestamp(buffer)?;

        self.nonce.encode(buffer)?;
        self.user_agent.encode(buffer)?;
        buffer.write_all(&self.start_height.to_le_bytes())?;
        buffer.write_all(&[self.relay as u8])?;

        Ok(())
    }

    fn decode(bytes: &mut Cursor<&[u8]>) -> io::Result<Self> {
        let version = ProtocolVersion::decode(bytes)?;
        let services = u64::from_le_bytes(read_n_bytes(bytes)?);
        let timestamp = read_timestamp(bytes)?;

        let addr_recv = NetworkAddr::decode_without_timestamp(bytes)?;
        let addr_from = NetworkAddr::decode_without_timestamp(bytes)?;

        let nonce = Nonce::decode(bytes)?;
        let user_agent = VarStr::decode(bytes)?;

        let start_height = u32::from_le_bytes(read_n_bytes(bytes)?);
        let relay = u8::from_le_bytes(read_n_bytes(bytes)?) != 0;

        Ok(Self {
            version,
            services,
            timestamp,
            addr_recv,
            addr_from,
            nonce,
            user_agent,
            start_height,
            relay,
        })
    }
}
