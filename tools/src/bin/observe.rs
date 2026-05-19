use messages::{Diagnostics, Notification};
use postcard::from_bytes;
use tokio::net::UdpSocket;

#[tokio::main]
async fn main() {
    let socket = UdpSocket::bind("0.0.0.0:12345").await.unwrap();

    let mut buffer = [0_u8; 1024];
    loop {
        let (_len, addr) = socket.recv_from(&mut buffer).await.unwrap();
        match from_bytes::<(Notification, Diagnostics)>(&buffer) {
            Ok((n, diag)) => {
                println!(
                    "{} @ {} ms with period {} ms took {} ms jitter {} ms",
                    addr,
                    diag.timestamp_us,
                    diag.period_in_us,
                    diag.execution_us,
                    diag.jitter_in_us
                );
                match n {
                    Notification::DO {
                        pins: [(pin, level)],
                    } => {
                        println!(
                            "    digital_out: pin {} {}",
                            pin,
                            match level {
                                false => "off",
                                true => "on",
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
