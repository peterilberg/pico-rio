use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{IpEndpoint, Ipv4Address};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

use messages::{Diagnostics, Notification};
use postcard::to_slice;
use {defmt_rtt as _, panic_probe as _};

use embassy_sync::channel::Receiver;

use crate::network;

type Packet = (IpEndpoint, Notification, Diagnostics);

#[embassy_executor::task]
pub async fn udp_output_task(
    stack: network::NStack,
    receiver: Receiver<
        'static,
        CriticalSectionRawMutex,
        (IpEndpoint, messages::Notification, messages::Diagnostics),
        16,
    >,
) {
    network::wait_for_network().await;

    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let mut rx_meta = [PacketMetadata::EMPTY; 8];
    let mut tx_meta = [PacketMetadata::EMPTY; 8];

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
        let (endpoint, msg, diag) = receiver.receive().await;

        log::info!("recv echo");
        let mut buf = [0_u8; 256];
        match to_slice(&(msg, diag), &mut buf) {
            Ok(x) => {
                match socket.send_to(x, endpoint).await {
                    Ok(()) => {
                        log::info!("sent to network");
                    }
                    Err(e) => {
                        log::info!("write error: {:?}", e);
                    }
                };
            }
            Err(e) => {
                log::info!("encoding failed {}", e);
            }
        }
    }
}
