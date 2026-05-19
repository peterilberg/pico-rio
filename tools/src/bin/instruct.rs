use messages::Command;
use postcard::to_slice;
use std::{env, net::SocketAddr};
use tokio::net::UdpSocket;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let args2 = args.iter().map(|x| x.as_str()).collect::<Vec<_>>();

    match args2[1..] {
        ["digital_out", "set", "25", level] => {
            let message = Command::SetDO {
                pin: 25,
                value: matches!(level, "on"),
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
