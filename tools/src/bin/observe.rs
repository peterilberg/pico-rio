use messages::{DigitalLevel, Kind, Message};
use postcard::{from_bytes, to_slice};
use tokio::net::UdpSocket;

#[tokio::main]
async fn main() {
    let socket = UdpSocket::bind("0.0.0.0:12345").await.unwrap();

    let mut buffer = [0_u8; 1024];
    loop {
        let (len, addr) = socket.recv_from(&mut buffer).await.unwrap();
        match from_bytes::<Message>(&buffer) {
            Ok(msg) => {
                println!(
                    "{} ({}) @ {} ms with period {} ms took {} ms jitter {} ms",
                    msg.sender, addr, msg.time_ms, msg.period_ms, msg.exec_ms, msg.jitter_ms
                );
                match msg.kind {
                    Kind::DigitalOut { pin_25 } => {
                        println!(
                            "    digital_out: pin_25 {}",
                            match pin_25 {
                                DigitalLevel::Off => "off",
                                DigitalLevel::On => "on",
                            }
                        );
                    }
                    _ => {}
                }
            }
            Err(x) => {
                println!("invalid message {}", x);
            }
        }
    }
}
