use postcard::{from_bytes, to_slice};
use serde::{Deserialize, Serialize};
use std::{env, net::SocketAddr};
use tokio::net::UdpSocket;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
enum Kind {
    Status,
    Echo([u8; 32]),
    DigitalOut { pin_25: DigitalLevel },
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
struct Message {
    sender: u32,
    time_ms: u64,
    exec_ms: u64,
    jitter_ms: u64,
    period_ms: u64,
    kind: Kind,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy)]
enum DigitalLevel {
    Off,
    On,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let args2 = args.iter().map(|x| x.as_str()).collect::<Vec<_>>();

    match args2[1..] {
        ["digital_out", "set", "25", level] => {
            let message = Message {
                sender: 0,
                time_ms: 0,
                exec_ms: 0,
                jitter_ms: 0,
                period_ms: 0,
                kind: Kind::DigitalOut {
                    pin_25: match level {
                        "on" => DigitalLevel::On,
                        _ => DigitalLevel::Off,
                    },
                },
            };

            let socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
            let mut buffer = [0_u8; 1024];
            let msg = to_slice(&message, &mut buffer);
            match msg {
                Err(x) => println!("encoding failed {}", x),
                Ok(msg) => {
                    let remote = "192.168.7.1:1234".parse::<SocketAddr>().unwrap();
                    match socket.send_to(msg, remote).await {
                        Ok(x) => println!("send command {}", x),
                        Err(x) => println!("fail command {}", x),
                    }
                }
            }
        }
        _ => println!("unknown command"),
    }
}
