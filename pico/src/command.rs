use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{IpEndpoint, Ipv4Address};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

use crate::digital_out;

use messages::Command;
use postcard::from_bytes;
use {defmt_rtt as _, panic_probe as _};

use embassy_sync::channel::{Channel, Sender};

use crate::network;

type Mailbox<T> = Channel<CriticalSectionRawMutex, T, 16>;

#[embassy_executor::task]
pub async fn udp_input_task(
    stack: network::NStack,
    digital_out: Sender<'static, CriticalSectionRawMutex, digital_out::Message, 16>,
) {
    network::wait_for_network().await;

    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let mut rx_meta = [PacketMetadata::EMPTY; 8];
    let mut tx_meta = [PacketMetadata::EMPTY; 8];
    let mut buf = [0; 4096];

    let local_endpoint = IpEndpoint::new(
        embassy_net::IpAddress::Ipv4(Ipv4Address::new(192, 168, 7, 1)),
        1234,
    );

    let mut socket = UdpSocket::new(
        stack.0,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
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
