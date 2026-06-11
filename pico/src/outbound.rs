use embassy_net::IpEndpoint;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use messages::{Content, Diagnostics, Info};
use postcard::to_slice;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use crate::network::{self, NetworkStack, SocketBuffers};

pub type Message = Info;

static INBOX: Channel<CriticalSectionRawMutex, Message, 16> = Channel::new();

pub async fn send(content: Content, diagnostics: Diagnostics) {
    INBOX
        .send(Message {
            content,
            diagnostics,
        })
        .await;
}

#[embassy_executor::task]
pub async fn task(stack: NetworkStack, endpoint: IpEndpoint) {
    network::wait_for_network().await;

    static BUFFERS: StaticCell<SocketBuffers> = StaticCell::new();
    let buffers = BUFFERS.init(SocketBuffers::default());
    let socket = stack.new_udp_socket(0, buffers);
    log::info!("outbound: endpoint {:?}", socket.endpoint());

    loop {
        let message = INBOX.receive().await;

        let mut buf = [0; 256];
        let Ok(bytes) = to_slice(&message, &mut buf) else {
            log::info!("outbound: encoding");
            continue;
        };

        match socket.send_to(bytes, endpoint).await {
            Ok(()) => {}
            Err(e) => {
                log::info!("outbound: error: {:?}", e);
            }
        };
    }
}
