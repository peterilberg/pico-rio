use messages::Command;
use postcard::{from_bytes, to_slice};
use serde::Deserialize;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use tokio::net::UdpSocket;
use tokio::time::{self, Duration};

pub fn parse_address(address: String) -> Result<SocketAddr, String> {
    match address.parse::<SocketAddr>() {
        Ok(address) => Ok(address),
        Err(error) => Err([
            error.to_string(),
            ": ".to_string(),
            address,
            ", expected a.b.c.d:port".to_string(),
        ]
        .join("")),
    }
}

pub struct Socket(UdpSocket);

impl Socket {
    pub async fn bind(port: u16) -> Result<Socket, String> {
        let address = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port);
        match UdpSocket::bind(address).await {
            Ok(socket) => Ok(Socket(socket)),
            Err(error) => Err(error.to_string()),
        }
    }

    pub async fn send(&self, destination: SocketAddr, buffer: &Buffer) -> Result<(), String> {
        let Socket(socket) = self;
        let Buffer(buffer) = buffer;
        match socket.send_to(buffer, destination).await {
            Ok(_) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }

    pub async fn recv(&self, buffer: &mut Buffer, wait: Duration) -> Result<SocketAddr, String> {
        let Socket(socket) = self;
        let Buffer(buffer) = buffer;

        let sleep = time::sleep(wait);
        tokio::pin!(sleep);

        tokio::select! {
            reply = socket.recv_from(buffer) => {
                match reply {
                    Ok((_, sender)) => Ok(sender),
                    Err(error) => Err(error.to_string()),
                }
            }
            _ = &mut sleep => {
                Err(String::from("time out - no messages"))
            }
        }
    }
}

pub struct Buffer([u8; 1024]);

impl Buffer {
    pub fn new() -> Self {
        Buffer([0; 1024])
    }

    pub fn encode(&mut self, command: &Command) -> Result<(), String> {
        let Buffer(buffer) = self;
        match to_slice(&command, buffer) {
            Ok(_) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }

    pub fn decode<'t, T>(&'t self) -> Result<T, String>
    where
        T: Deserialize<'t>,
    {
        let Buffer(buffer) = self;
        match from_bytes::<T>(buffer) {
            Ok(result) => Ok(result),
            Err(error) => Err(error.to_string()),
        }
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}
