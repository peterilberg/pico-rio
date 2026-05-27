use embassy_net::{IpEndpoint, Ipv4Address};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Sender};

use messages::Command;
use postcard::from_bytes;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use crate::digital_out;
use crate::network::{self, SocketBuffers};

type Mailbox<T> = Channel<CriticalSectionRawMutex, T, 16>;

#[embassy_executor::task]
pub async fn udp_input_task(
    stack: network::NetworkStack,
    digital_out: Sender<'static, CriticalSectionRawMutex, digital_out::Message, 16>,
) {
    network::wait_for_network().await;

    static BUFFERS: StaticCell<network::SocketBuffers> = StaticCell::new();
    let buffers = BUFFERS.init(SocketBuffers::default());
    let mut socket = stack.udp_socket(buffers);

    let local_endpoint = IpEndpoint::new(
        embassy_net::IpAddress::Ipv4(Ipv4Address::new(192, 168, 7, 1)),
        1234,
    );

    log::info!("Listening on UDP:1234...");
    match socket.bind(local_endpoint) {
        Ok(()) => {
            log::info!("bound to {}", local_endpoint);
        }
        Err(e) => {
            log::info!("bind {:?}", e);
        }
    }
    log::info!("local local_endpoint {:?}", socket.endpoint());

    let mut buf = [0; 4096];

    loop {
        let (n, _meta) = match socket.recv_from(&mut buf).await {
            Ok((0, _)) => {
                log::info!("read EOF");
                continue;
            }
            Ok((n, meta)) => {
                log::info!("connection from {:?}", meta.endpoint);
                log::info!("reading {}", n);
                (n, meta)
            }
            Err(e) => {
                log::info!("read error: {:?}", e);
                continue;
            }
        };

        for (i, x) in buf[..n].iter().enumerate() {
            log::info!("rxd {} {}", i, x);
        }

        log::info!("decoding message");
        let msg = match from_bytes::<Command>(&buf[..]) {
            Ok(h) => {
                log::info!("message {:?}", h);
                h
            }
            Err(x) => {
                log::info!("invalid message ignored {}", x);
                continue;
            }
        };

        match msg {
            Command::SetDO { pin, value } => {
                log::info!("setting pin {} to {}", pin, value);
                digital_out
                    .send(digital_out::Message::Set { pin, value })
                    .await;
            }
            _ => {
                log::info!("ignoring message {:?}", msg);
            }
        }
    }
}
