use messages::Command;
use std::fmt;

pub type Strings = &'static [&'static str];

pub trait Instruction {
    fn prefix(&self) -> Strings;
    fn arguments(&self) -> Strings;
    fn run(&self, arguments: &[String]) -> Result<Vec<Command>, String>;
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

        if matching_words == prefix_len {
            if command.len() == prefix_len + args_len {
                return Match::Full(*instruction);
            } else {
                return Match::Partial(*instruction);
            }
        }
    }

    Match::None
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
