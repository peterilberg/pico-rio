pub mod instruction;
pub mod logger;
pub mod network;

pub const PICO_ADDRESS_USAGE: &str = "
Set environment variable PICO_ADDRESS to the address and port
of your pico. By default: PICO_ADDRESS=192.168.7.1:1234";
