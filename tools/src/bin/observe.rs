use messages::{Content, Diagnostics, Info};
use postcard::from_bytes;
use std::env::Args;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use tokio::net::UdpSocket;

use tools::logger::Logger;

#[tokio::main]
async fn main() {
    let arguments = std::env::args();
    let Config { port } = Config::build(arguments).unwrap_or_else(|error| {
        eprintln!("error: {}", error);
        std::process::exit(1);
    });

    let address = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port);
    let server = UdpSocket::bind(address).await.unwrap_or_else(|error| {
        eprintln!("error: {}", error);
        std::process::exit(1);
    });

    let mut buffer = [0_u8; 1024];
    loop {
        let (_, sender) = server.recv_from(&mut buffer).await.unwrap();
        match from_bytes::<Info>(&buffer) {
            Ok(Info {
                content,
                diagnostics,
            }) => dispatch(sender, diagnostics, content),
            Err(error) => {
                Logger::error(sender);
                println!("invalid message: {}", error);
            }
        }
    }
}

struct Config {
    port: u16,
}

impl Config {
    fn build(mut arguments: Args) -> Result<Self, String> {
        arguments.next(); // ignore executable name

        let port = match arguments.next() {
            Some(arg) => arg,
            None => {
                let message = String::from("missing port number");
                return Self::error(message);
            }
        };

        let port = match port.parse::<u16>() {
            Ok(port) => port,
            Err(error) => {
                let message = format!("invalid port number {}: {}", port, error);
                return Self::error(message);
            }
        };

        Ok(Config { port })
    }

    fn error(message: String) -> Result<Self, String> {
        let usage = String::from("usage: local-port");
        Err([message, usage].join("\n"))
    }
}

fn dispatch(sender: SocketAddr, diagnostics: Diagnostics, content: Content) {
    match content {
        Content::Pong => {
            Logger::error(sender);
            println!("unexpected pong");
        }
        Content::DO { pins } => {
            let logger = Logger::new(sender, "digital out");
            logger.digital_out(pins, diagnostics);
        }
    }
}
