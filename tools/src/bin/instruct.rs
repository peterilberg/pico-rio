use messages::{Command, Info, Value};
use std::env::Args;
use std::time::Duration;

use heapless;
use tools::instruction::{Instruction, Match, Strings, find_instruction};
use tools::network::{Buffer, Socket, parse_address};

#[tokio::main]
async fn main() {
    let Config { command } = Config::build(std::env::args()).unwrap_or_else(|error| {
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
        &ShowBangBang,
        &HideBangBang,
        &ClearDisplay,
        &DisplayText,
        &DisplayAnalog,
        &DisplayOffOn,
        &DisplayOnOff,
        &ExampleWaterTank,
    ];

    match find_instruction(&instructions, &command) {
        Match::Full(instruction) => {
            match process_instruction(instruction, &command).await {
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
    command: Vec<String>,
}

impl Config {
    fn build(mut arguments: Args) -> Result<Self, String> {
        arguments.next(); // ignore executable name

        let command = arguments.collect::<Vec<_>>();
        if command.len() == 0 {
            Self::error("missing command".to_string())?;
        }

        Ok(Config { command })
    }

    fn error(message: String) -> Result<Self, String> {
        const USAGE: &str = "usage: COMMAND
Use command 'help' to show available commands.

Set environment variable PICO_ADDRESS to the address and port
of your pico. By default: PICO_ADDRESS=192.168.7.1:1234";

        let usage = String::from(USAGE);
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
) -> Result<(), String> {
    let first_argument = instruction.prefix().len();
    let arguments = &command[first_argument..];
    let commands = instruction.run(arguments)?;

    let destination = match std::env::var("PICO_ADDRESS") {
        Ok(value) => value,
        Err(_) => String::from("192.168.7.1:1234"),
    };

    let destination = parse_address(destination)?;
    let socket = Socket::bind(destination.port()).await?;

    for command in commands {
        let mut buffer = Buffer::new();
        buffer.encode(&command)?;
        socket.send(destination, &buffer).await?;

        if let Command::Ping = command {
            let wait = Duration::from_secs(3);
            let sender = socket.recv(&mut buffer, wait).await?;
            let Info { content, .. } = buffer.decode::<Info>()?;
            println!("{}: {:?}", sender, content);
        }
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

    fn run(&self, _arguments: &[String]) -> Result<Vec<Command>, String> {
        Ok(vec![Command::Ping])
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

    fn run(&self, arguments: &[String]) -> Result<Vec<Command>, String> {
        let pin = get_pin(&arguments[0])?;
        let value = get_on_off(&arguments[1])?;
        Ok(vec![Command::SetDO { pin, value }])
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

    fn run(&self, arguments: &[String]) -> Result<Vec<Command>, String> {
        let pin = get_pin(&arguments[0])?;
        let value = get_number(&arguments[1])?;
        Ok(vec![Command::SetAO { pin, value }])
    }
}

struct StartBangBang;

impl Instruction for StartBangBang {
    fn prefix(&self) -> Strings {
        &["bang", "bang", "start"]
    }

    fn arguments(&self) -> Strings {
        &[]
    }

    fn run(&self, _arguments: &[String]) -> Result<Vec<Command>, String> {
        Ok(vec![Command::BangBangStart])
    }
}

struct StopBangBang;

impl Instruction for StopBangBang {
    fn prefix(&self) -> Strings {
        &["bang", "bang", "stop"]
    }

    fn arguments(&self) -> Strings {
        &[]
    }

    fn run(&self, _arguments: &[String]) -> Result<Vec<Command>, String> {
        Ok(vec![Command::BangBangStop])
    }
}

struct SetBangBangInput;

impl Instruction for SetBangBangInput {
    fn prefix(&self) -> Strings {
        &["bang", "bang", "input"]
    }

    fn arguments(&self) -> Strings {
        &["PIN"]
    }

    fn run(&self, arguments: &[String]) -> Result<Vec<Command>, String> {
        let pin = get_pin(&arguments[0])?;
        Ok(vec![Command::BangBangInput { pin }])
    }
}

struct SetBangBangOutput;

impl Instruction for SetBangBangOutput {
    fn prefix(&self) -> Strings {
        &["bang", "bang", "output"]
    }

    fn arguments(&self) -> Strings {
        &["PIN"]
    }

    fn run(&self, arguments: &[String]) -> Result<Vec<Command>, String> {
        let pin = get_pin(&arguments[0])?;
        Ok(vec![Command::BangBangOutput { pin }])
    }
}

struct SetBangBangLowerLimit;

impl Instruction for SetBangBangLowerLimit {
    fn prefix(&self) -> Strings {
        &["bang", "bang", "lower"]
    }

    fn arguments(&self) -> Strings {
        &["0-100"]
    }

    fn run(&self, arguments: &[String]) -> Result<Vec<Command>, String> {
        let value = get_number(&arguments[0])?;
        Ok(vec![Command::BangBangLowerLimit { value }])
    }
}

struct SetBangBangUpperLimit;

impl Instruction for SetBangBangUpperLimit {
    fn prefix(&self) -> Strings {
        &["bang", "bang", "upper"]
    }

    fn arguments(&self) -> Strings {
        &["0-100"]
    }

    fn run(&self, arguments: &[String]) -> Result<Vec<Command>, String> {
        let value = get_number(&arguments[0])?;
        Ok(vec![Command::BangBangUpperLimit { value }])
    }
}

struct ShowBangBang;

impl Instruction for ShowBangBang {
    fn prefix(&self) -> Strings {
        &["bang", "bang", "show"]
    }

    fn arguments(&self) -> Strings {
        &[]
    }

    fn run(&self, _arguments: &[String]) -> Result<Vec<Command>, String> {
        Ok(vec![Command::BangBangShow])
    }
}

struct HideBangBang;

impl Instruction for HideBangBang {
    fn prefix(&self) -> Strings {
        &["bang", "bang", "hide"]
    }

    fn arguments(&self) -> Strings {
        &[]
    }

    fn run(&self, _arguments: &[String]) -> Result<Vec<Command>, String> {
        Ok(vec![Command::BangBangHide])
    }
}

struct ClearDisplay;

impl Instruction for ClearDisplay {
    fn prefix(&self) -> Strings {
        &["clear", "display"]
    }

    fn arguments(&self) -> Strings {
        &[]
    }

    fn run(&self, _arguments: &[String]) -> Result<Vec<Command>, String> {
        Ok(vec![Command::ClearDisplay])
    }
}
struct DisplayText;

impl Instruction for DisplayText {
    fn prefix(&self) -> Strings {
        &["display", "label"]
    }

    fn arguments(&self) -> Strings {
        &["LABEL"]
    }

    fn run(&self, arguments: &[String]) -> Result<Vec<Command>, String> {
        let label = get_label(&arguments[0])?;
        Ok(vec![Command::AddLine {
            label,
            value: Value::None,
        }])
    }
}

struct DisplayAnalog;

impl Instruction for DisplayAnalog {
    fn prefix(&self) -> Strings {
        &["display", "analog"]
    }

    fn arguments(&self) -> Strings {
        &["LABEL", "PIN"]
    }

    fn run(&self, arguments: &[String]) -> Result<Vec<Command>, String> {
        let label = get_label(&arguments[0])?;
        let pin = get_pin(&arguments[1])?;
        Ok(vec![Command::AddLine {
            label,
            value: Value::Analog(pin),
        }])
    }
}

struct DisplayOffOn;

impl Instruction for DisplayOffOn {
    fn prefix(&self) -> Strings {
        &["display", "off", "on"]
    }

    fn arguments(&self) -> Strings {
        &["LABEL", "PIN"]
    }

    fn run(&self, arguments: &[String]) -> Result<Vec<Command>, String> {
        let label = get_label(&arguments[0])?;
        let pin = get_pin(&arguments[1])?;
        Ok(vec![Command::AddLine {
            label,
            value: Value::OffOn(pin),
        }])
    }
}

struct DisplayOnOff;

impl Instruction for DisplayOnOff {
    fn prefix(&self) -> Strings {
        &["display", "on", "off"]
    }

    fn arguments(&self) -> Strings {
        &["LABEL", "PIN"]
    }

    fn run(&self, arguments: &[String]) -> Result<Vec<Command>, String> {
        let label = get_label(&arguments[0])?;
        let pin = get_pin(&arguments[1])?;
        Ok(vec![Command::AddLine {
            label,
            value: Value::OnOff(pin),
        }])
    }
}

struct ExampleWaterTank;

impl Instruction for ExampleWaterTank {
    fn prefix(&self) -> Strings {
        &["example", "water", "tank"]
    }

    fn arguments(&self) -> Strings {
        &[]
    }

    fn run(&self, _arguments: &[String]) -> Result<Vec<Command>, String> {
        Ok(vec![
            Command::ClearDisplay,
            add_line("Water tank", Value::None),
            add_line("Pump", Value::Analog(26)),
            add_line("Fill level", Value::Analog(27)),
            add_line("Source (NC)", Value::OffOn(20)),
            add_line("Drain  (NO)", Value::OnOff(19)),
            Command::BangBangInput { pin: 27 },
            Command::BangBangOutput { pin: 6 },
            Command::BangBangLowerLimit { value: 45 },
            Command::BangBangUpperLimit { value: 50 },
        ])
    }
}

fn add_line(label: &str, value: Value) -> Command {
    let label = match heapless::String::<16>::try_from(label) {
        Ok(string) => string,
        Err(_) => heapless::String::new(),
    };
    Command::AddLine { label, value }
}

fn get_pin(argument: &String) -> Result<u8, String> {
    match argument.parse::<u8>() {
        Ok(pin) => Ok(pin),
        Err(error) => Err(format!("invalid pin: {}", error)),
    }
}

fn get_on_off(argument: &String) -> Result<bool, String> {
    Ok(matches!(argument.as_str(), "on"))
}

fn get_number(argument: &String) -> Result<u8, String> {
    match argument.parse::<u8>() {
        Ok(pin) => Ok(pin),
        Err(error) => Err(format!("invalid number: {}", error)),
    }
}

fn get_label(argument: &String) -> Result<heapless::String<16>, String> {
    match heapless::String::<16>::try_from(argument.as_str()) {
        Ok(string) => Ok(string),
        Err(error) => return Err(format!("invalid label: {}", error)),
    }
}
