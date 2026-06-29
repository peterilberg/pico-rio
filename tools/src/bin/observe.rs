use messages::{Command, Content, Diagnostics, Info};
use std::env::Args;
use std::net::SocketAddr;
use std::time::Duration;

use tools::PICO_ADDRESS_USAGE;
use tools::logger::Logger;
use tools::network::{Buffer, Socket, get_pico_address, parse_address};

#[tokio::main]
async fn main() {
    let arguments = std::env::args();
    let Config { destination } = Config::build(arguments).unwrap_or_else(|error| {
        eprintln!("error: {}", error);
        std::process::exit(1);
    });

    let server = Socket::bind(0).await.unwrap_or_else(|error| {
        eprintln!("error: {}", error);
        std::process::exit(1);
    });

    let command = Command::Subscribe;
    let mut buffer = Buffer::new();
    // TODO don't blindly unwrap
    buffer.encode(&command).unwrap();
    server.send(destination, &buffer).await.unwrap();

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
    destination: SocketAddr,
}

impl Config {
    fn build(mut arguments: Args) -> Result<Self, String> {
        arguments.next(); // ignore executable name

        if arguments.len() > 0 {
            return Self::error("command does not expect arguments".to_string());
        };

        match parse_address(get_pico_address()) {
            Ok(destination) => Ok(Config { destination }),
            Err(error) => Self::error(error.to_string()),
        }
    }

    fn error(message: String) -> Result<Self, String> {
        let address = String::from(PICO_ADDRESS_USAGE);
        Err([message, address].join("\n"))
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
