use messages::{Command, Content, Info};
use postcard::{from_bytes, to_slice};
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::time::{self, Duration};

pub struct Socket(UdpSocket);

impl Socket {
    pub async fn get() -> Result<Socket, String> {
        match UdpSocket::bind("0.0.0.0:0").await {
            Ok(socket) => Ok(Socket(socket)),
            Err(error) => Err(error.to_string()),
        }
    }

    pub async fn send(&self, destination: SocketAddr, data: &[u8]) -> Result<(), String> {
        let Socket(socket) = self;
        match socket.send_to(data, destination).await {
            Ok(_) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }

    pub async fn recv(&self, destination: SocketAddr, buffer: &mut [u8]) -> Result<(), String> {
        let Socket(socket) = self;

        let sleep = time::sleep(Duration::from_secs(3));
        tokio::pin!(sleep);

        tokio::select! {
            reply = socket.recv(buffer) => {
                match reply {
                    Ok(_) => Ok(()),
                    Err(error) => Err(error.to_string()),
                }
            }
            _ = &mut sleep => {
                Err(format!("no reply from {}", destination))
            }
        }
    }
}

pub fn encode_command<'buf>(
    command: &Command,
    buffer: &'buf mut [u8],
) -> Result<&'buf [u8], String> {
    match to_slice(&command, buffer) {
        Ok(message) => Ok(message),
        Err(error) => Err(error.to_string()),
    }
}

pub fn decode_content(buffer: &[u8]) -> Result<Content, String> {
    match from_bytes::<Info>(buffer) {
        Ok(Info { content, .. }) => Ok(content),
        Err(error) => Err(error.to_string()),
    }
}

pub fn parse_address(address: String) -> Result<SocketAddr, String> {
    match address.parse::<SocketAddr>() {
        Ok(address) => Ok(address),
        Err(error) => Err(error.to_string()),
    }
}
