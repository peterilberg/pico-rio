use embassy_net::IpEndpoint;
use messages::Info;
use postcard::to_slice;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use crate::mailbox::Inbox;
use crate::network::{self, NetworkStack, SocketBuffers};

pub type Message = Info;

#[embassy_executor::task]
pub async fn task(stack: NetworkStack, endpoint: IpEndpoint, inbox: Inbox<Message>) {
    network::wait_for_network().await;

    static BUFFERS: StaticCell<SocketBuffers> = StaticCell::new();
    let buffers = BUFFERS.init(SocketBuffers::default());
    let socket = stack.new_udp_socket(0, buffers);
    log::info!("outbound: endpoint {:?}", socket.endpoint());

    loop {
        let message = inbox.receive().await;

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
