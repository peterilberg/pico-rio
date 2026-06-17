use messages::{Command, Info};
use std::env::Args;
use std::time::Duration;

use tools::instruction::{Instruction, Match, Strings, find_instruction};
use tools::network::{Buffer, Socket, parse_address};

#[tokio::main]
async fn main() {
    let Config {
        destination,
        command,
    } = Config::build(std::env::args()).unwrap_or_else(|error| {
        eprintln!("error: {}", error);
        std::process::exit(1);
    });

    let instructions: [&dyn Instruction; _] = [
        &PingPong,
        &SetDigitalOut,
        &SetAnalogOut,
        &StartBangBang,
        &StopBangBang,
        &SetBangBangInput,
        &SetBangBangOutput,
        &SetBangBangLowerLimit,
        &SetBangBangUpperLimit,
    ];

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

    let mut buffer = Buffer::new();
    buffer.encode(&command)?;

    let destination = parse_address(destination)?;
    let socket = Socket::bind(destination.port()).await?;
    socket.send(destination, &buffer).await?;

    if let Command::Ping = command {
        let wait = Duration::from_secs(3);
        let sender = socket.recv(&mut buffer, wait).await?;
        let Info { content, .. } = buffer.decode::<Info>()?;
        println!("{}: {:?}", sender, content);
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
        &["digital"]
    }

    fn arguments(&self) -> Strings {
        &["PIN", "on|off"]
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

struct SetAnalogOut;

impl Instruction for SetAnalogOut {
    fn prefix(&self) -> Strings {
        &["analog"]
    }

    fn arguments(&self) -> Strings {
        &["PIN", "0-100"]
    }

    fn run(&self, arguments: &[String]) -> Result<Command, String> {
        let pin = match arguments[0].parse::<u8>() {
            Ok(pin) => pin,
            Err(error) => return Err(error.to_string()),
        };
        let value = match arguments[1].parse::<u8>() {
            Ok(value) => value,
            Err(error) => return Err(error.to_string()),
        };
        Ok(Command::SetAO { pin, value })
    }
}

struct StartBangBang;

impl Instruction for StartBangBang {
    fn prefix(&self) -> Strings {
        &["bang_bang", "start"]
    }

    fn arguments(&self) -> Strings {
        &[]
    }

    fn run(&self, _arguments: &[String]) -> Result<Command, String> {
        Ok(Command::BangBangStart)
    }
}

struct StopBangBang;

impl Instruction for StopBangBang {
    fn prefix(&self) -> Strings {
        &["bang_bang", "stop"]
    }

    fn arguments(&self) -> Strings {
        &[]
    }

    fn run(&self, _arguments: &[String]) -> Result<Command, String> {
        Ok(Command::BangBangStop)
    }
}

struct SetBangBangInput;

impl Instruction for SetBangBangInput {
    fn prefix(&self) -> Strings {
        &["bang_bang", "input"]
    }

    fn arguments(&self) -> Strings {
        &["PIN"]
    }

    fn run(&self, arguments: &[String]) -> Result<Command, String> {
        let pin = match arguments[0].parse::<u8>() {
            Ok(pin) => pin,
            Err(error) => return Err(error.to_string()),
        };
        Ok(Command::BangBangInput { pin })
    }
}

struct SetBangBangOutput;

impl Instruction for SetBangBangOutput {
    fn prefix(&self) -> Strings {
        &["bang_bang", "output"]
    }

    fn arguments(&self) -> Strings {
        &["PIN"]
    }

    fn run(&self, arguments: &[String]) -> Result<Command, String> {
        let pin = match arguments[0].parse::<u8>() {
            Ok(pin) => pin,
            Err(error) => return Err(error.to_string()),
        };
        Ok(Command::BangBangOutput { pin })
    }
}

struct SetBangBangLowerLimit;

impl Instruction for SetBangBangLowerLimit {
    fn prefix(&self) -> Strings {
        &["bang_bang", "lower"]
    }

    fn arguments(&self) -> Strings {
        &["0-100"]
    }

    fn run(&self, arguments: &[String]) -> Result<Command, String> {
        let value = match arguments[0].parse::<u8>() {
            Ok(pin) => pin,
            Err(error) => return Err(error.to_string()),
        };
        Ok(Command::BangBangLowerLimit { value })
    }
}

struct SetBangBangUpperLimit;

impl Instruction for SetBangBangUpperLimit {
    fn prefix(&self) -> Strings {
        &["bang_bang", "upper"]
    }

    fn arguments(&self) -> Strings {
        &["0-100"]
    }

    fn run(&self, arguments: &[String]) -> Result<Command, String> {
        let value = match arguments[0].parse::<u8>() {
            Ok(pin) => pin,
            Err(error) => return Err(error.to_string()),
        };
        Ok(Command::BangBangUpperLimit { value })
    }
}
