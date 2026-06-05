use messages::Command;
use std::env::Args;

use tools::instruction::{Instruction, Match, Strings, find_instruction};
use tools::network::{Socket, decode_content, encode_command, parse_address};

#[tokio::main]
async fn main() {
    let Config {
        destination,
        command,
    } = Config::build(std::env::args()).unwrap_or_else(|error| {
        eprintln!("error: {}", error);
        std::process::exit(1);
    });

    let instructions: [&dyn Instruction; _] = [&PingPong, &SetDigitalOut];

    match find_instruction(&instructions, &command) {
        Match::Full(instruction) => {
            match process_instruction(instruction, &command, destination).await {
                Ok(_) => (),
                Err(err) => println!("error: {}", err),
            };
        }
        Match::Partial(instruction) => {
            usage(instruction);
        }
        Match::None => {
            list_instructions(&instructions);
        }
    }
}

struct Config {
    destination: String,
    command: Vec<String>,
}

impl Config {
    fn build(mut arguments: Args) -> Result<Self, String> {
        arguments.next(); // ignore executable name

        let destination = match arguments.next() {
            Some(arg) => arg,
            None => {
                let message = String::from("missing destination:port");
                return Self::error(message);
            }
        };

        Ok(Config {
            destination,
            command: arguments.collect(),
        })
    }

    fn error(message: String) -> Result<Self, String> {
        let usage = String::from("usage: destination:port COMMAND");
        Err([message, usage].join("\n"))
    }
}

fn usage(instruction: &'static dyn Instruction) {
    println!("usage: {}", instruction);
}

fn list_instructions(instructions: &[&'static dyn Instruction]) {
    println!("Available commands:");
    for instruction in instructions {
        println!("    {}", instruction);
    }
}

async fn process_instruction(
    instruction: &'static dyn Instruction,
    command: &[String],
    destination: String,
) -> Result<(), String> {
    let first_argument = instruction.prefix().len();
    let arguments = &command[first_argument..];
    let command = instruction.run(arguments)?;

    let mut buffer = [0_u8; 1024];
    let data = encode_command(&command, &mut buffer)?;

    let destination = parse_address(destination)?;
    let socket = Socket::get().await?;
    socket.send(destination, data).await?;

    if let Command::Ping = command {
        socket.recv(destination, &mut buffer).await?;
        let content = decode_content(&buffer)?;
        println!("{}: {:?}", destination, content);
    }
    Ok(())
}

struct PingPong;

impl Instruction for PingPong {
    fn prefix(&self) -> Strings {
        &["ping"]
    }

    fn arguments(&self) -> Strings {
        &[]
    }

    fn run(&self, _arguments: &[String]) -> Result<Command, String> {
        Ok(Command::Ping)
    }
}

struct SetDigitalOut;

impl Instruction for SetDigitalOut {
    fn prefix(&self) -> Strings {
        &["digital", "out", "pin"]
    }

    fn arguments(&self) -> Strings {
        &["PIN", "(on|off)"]
    }

    fn run(&self, arguments: &[String]) -> Result<Command, String> {
        let pin = match arguments[0].parse::<u8>() {
            Ok(pin) => pin,
            Err(error) => return Err(error.to_string()),
        };
        let value = matches!(&*arguments[1], "on");
        Ok(Command::SetDO { pin, value })
    }
}
