use messages::{BangBang, Diagnostics, Mode, Pins};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

pub struct Logger<'task> {
    ip: IpAddr,
    task: &'task str,
}

impl<'task> Logger<'task> {
    pub fn new(address: SocketAddr, task: &'task str) -> Self {
        Logger {
            task,
            ip: address.ip(),
        }
    }

    pub fn error(address: SocketAddr) {
        Logger::new(address, "ERROR: ").prefix();
    }

    pub fn separator() {
        println!();
    }

    pub fn prefix(&self) {
        print!("{}: {} ", self.ip, self.task);
    }

    pub fn diagnostics(&self, diagnostics: Diagnostics) {
        self.prefix();
        println!(
            "at {:?} (+ {:?}) with period {:?} took {:?}",
            Duration::from_micros(diagnostics.timestamp_us),
            Duration::from_micros(diagnostics.jitter_in_us),
            Duration::from_micros(diagnostics.period_in_us),
            Duration::from_micros(diagnostics.execution_us),
        );
    }

    pub fn digital_in(&self, pins: Pins<bool>, diagnostics: Diagnostics) {
        self.diagnostics(diagnostics);
        for (pin, level) in pins {
            self.prefix();
            println!("pin {}: {}", pin, get_level(level),);
        }
    }

    pub fn digital_out(&self, pins: Pins<bool>, diagnostics: Diagnostics) {
        self.diagnostics(diagnostics);
        for (pin, level) in pins {
            self.prefix();
            println!("pin {}: {}", pin, get_level(level),);
        }
    }

    pub fn analog_in(&self, pins: Pins<u8>, diagnostics: Diagnostics) {
        self.diagnostics(diagnostics);
        for (pin, value) in pins {
            self.prefix();
            println!("pin {}: {}", pin, value);
        }
    }

    pub fn analog_out(&self, pins: Pins<u8>, diagnostics: Diagnostics) {
        self.diagnostics(diagnostics);
        for (pin, value) in pins {
            self.prefix();
            println!("pin {}: {}", pin, value);
        }
    }

    pub fn bang_bang(&self, settings: BangBang, diagnostics: Diagnostics) {
        self.diagnostics(diagnostics);
        self.prefix();
        println!("mode:        {}", get_mode(settings.mode));
        self.prefix();
        println!("input pin:   {}", settings.input_pin);
        self.prefix();
        println!("output pin:  {}", settings.output_pin);
        self.prefix();
        println!("lower limit: {}", settings.lower_limit);
        self.prefix();
        println!("upper limit: {}", settings.upper_limit);
    }
}

fn get_level(level: bool) -> &'static str {
    match level {
        false => "off",
        true => "on",
    }
}

fn get_mode(mode: Mode) -> &'static str {
    match mode {
        Mode::Off => "off",
        Mode::Running => "running",
        Mode::Waiting => "waiting",
    }
}
