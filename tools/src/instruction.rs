use messages::Command;
use postcard::to_slice;
use std::fmt;
use std::net::SocketAddr;
use tokio::net::UdpSocket;

pub type Strings = &'static [&'static str];

pub trait Instruction {
    fn prefix(&self) -> Strings;
    fn arguments(&self) -> Strings;
    fn run(&self, arguments: &[String]) -> Result<Command, String>;
}

pub enum Match<'i> {
    None,
    Partial(&'i dyn Instruction),
    Full(&'i dyn Instruction),
}

pub fn find_instruction<'i>(instructions: &[&'i dyn Instruction], command: &[String]) -> Match<'i> {
    for instruction in instructions {
        let matching_words = command
            .iter()
            .zip(instruction.prefix())
            .take_while(|(a, b)| a == b)
            .count();

        let prefix_len = instruction.prefix().len();
        let args_len = instruction.arguments().len();

        if matching_words == prefix_len && command.len() == prefix_len + args_len {
            return Match::Full(*instruction);
        } else if matching_words > 0 {
            return Match::Partial(*instruction);
        }
    }

    Match::None
}

pub async fn send_command(destination: String, command: Command) -> Result<(), String> {
    let socket = match UdpSocket::bind("0.0.0.0:0").await {
        Ok(socket) => socket,
        Err(error) => return Err(error.to_string()),
    };

    let mut buffer = [0_u8; 1024];
    let message = match to_slice(&command, &mut buffer) {
        Ok(message) => message,
        Err(error) => return Err(error.to_string()),
    };

    let remote = destination.parse::<SocketAddr>().unwrap();
    match socket.send_to(message, remote).await {
        Ok(_) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

impl fmt::Display for dyn Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for word in self.prefix() {
            write!(f, "{} ", word)?;
        }
        for word in self.arguments() {
            write!(f, "{} ", word)?;
        }
        Ok(())
    }
}
