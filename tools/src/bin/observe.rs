use messages::{Content, Diagnostics, Info};
use std::env::Args;
use std::net::SocketAddr;
use std::time::Duration;

use tools::logger::Logger;
use tools::network::{Buffer, Socket};

#[tokio::main]
async fn main() {
    let arguments = std::env::args();
    let Config { port } = Config::build(arguments).unwrap_or_else(|error| {
        eprintln!("error: {}", error);
        std::process::exit(1);
    });

    let server = Socket::bind(port).await.unwrap_or_else(|error| {
        eprintln!("error: {}", error);
        std::process::exit(1);
    });

    let mut buffer = Buffer::new();
    loop {
        let wait = Duration::from_secs(5);
        let Ok(sender) = server.recv(&mut buffer, wait).await else {
            continue;
        };

        match buffer.decode::<Info>() {
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
        Content::DI { pins } => {
            let logger = Logger::new(sender, "digital in");
            logger.digital_in(pins, diagnostics);
        }
        Content::DO { pins } => {
            let logger = Logger::new(sender, "digital out");
            logger.digital_out(pins, diagnostics);
        }
        Content::AI { pins } => {
            let logger = Logger::new(sender, "analog in");
            logger.analog_in(pins, diagnostics);
        }
        Content::AO { pins } => {
            let logger = Logger::new(sender, "analog out");
            logger.analog_out(pins, diagnostics);
        }
        Content::BangBang { settings } => {
            let logger = Logger::new(sender, "bang bang");
            logger.bang_bang(settings, diagnostics);
        }
    };
    Logger::separator();
}
