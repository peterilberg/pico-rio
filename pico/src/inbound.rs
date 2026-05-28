use embassy_net::IpEndpoint;
use embassy_net::udp::UdpSocket;
use embassy_time::Instant;
use messages::{Command, Content, Diagnostics, Info};
use postcard::{from_bytes, to_slice};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use crate::digital_out;
use crate::mailbox::Outbox;
use crate::network::{self, SocketBuffers};

#[embassy_executor::task]
pub async fn task(
    stack: network::NetworkStack,
    port: u16,
    digital_out: Outbox<digital_out::Message>,
) {
    network::wait_for_network().await;

    static BUFFERS: StaticCell<network::SocketBuffers> = StaticCell::new();
    let buffers = BUFFERS.init(SocketBuffers::default());
    let socket = stack.new_udp_socket(port, buffers);
    log::info!("inbound: endpoint {:?}", socket.endpoint());

    loop {
        let Some((endpoint, command)) = wait_for_command(&socket).await else {
            continue;
        };

        match command {
            Command::Ping => {
                ping_pong(&socket, endpoint).await;
            }
            Command::SetDO { pin, value } => {
                set_do(digital_out, pin, value).await;
            }
            command => {
                log::info!("inbound: ignored command {:?}", command);
            }
        }
    }
}

async fn wait_for_command(socket: &UdpSocket<'_>) -> Option<(IpEndpoint, Command)> {
    let mut buf = [0; 4096];

    let endpoint = match socket.recv_from(&mut buf).await {
        Ok((0, _)) => return None,
        Ok((_, meta)) => {
            log::info!("inbound: message from {:?}", meta.endpoint);
            meta.endpoint
        }
        Err(e) => {
            log::info!("inbound: read error: {:?}", e);
            return None;
        }
    };

    match from_bytes::<Command>(&buf[..]) {
        Ok(command) => Some((endpoint, command)),
        Err(e) => {
            log::info!("inbound: invalid command {:?}", e);
            None
        }
    }
}

async fn ping_pong(socket: &UdpSocket<'_>, endpoint: IpEndpoint) {
    let message = Info {
        content: Content::Pong,
        diagnostics: Diagnostics {
            timestamp_us: Instant::now().as_micros(),
            execution_us: 0,
            jitter_in_us: 0,
            period_in_us: 0,
        },
    };

    let mut buf = [0; 256];
    let Ok(bytes) = to_slice(&message, &mut buf) else {
        log::info!("inbound: error: pong");
        return;
    };

    match socket.send_to(bytes, endpoint).await {
        Ok(()) => {}
        Err(e) => {
            log::info!("inbound: error: {:?}", e);
        }
    };
}

async fn set_do(digital_out: Outbox<digital_out::Message>, pin: u8, value: bool) {
    digital_out
        .send(digital_out::Message::Set { pin, value })
        .await;
}
