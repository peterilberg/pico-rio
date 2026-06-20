use embassy_net::{IpEndpoint, udp::UdpSocket};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use messages::{Content, Diagnostics, Info};
use postcard::to_slice;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use crate::network::{self, NetworkStack, SocketBuffers};

enum Message {
    Info { info: Info },
    Endpoint { endpoint: IpEndpoint },
}

static INBOX: Channel<CriticalSectionRawMutex, Message, 16> = Channel::new();

pub async fn send(content: Content, diagnostics: Diagnostics) {
    let info = Info {
        content,
        diagnostics,
    };
    INBOX.send(Message::Info { info }).await;
}

pub async fn subscribe(endpoint: IpEndpoint) {
    INBOX.send(Message::Endpoint { endpoint }).await;
}

#[embassy_executor::task]
pub async fn task(stack: NetworkStack) {
    network::wait_for_network().await;

    static BUFFERS: StaticCell<SocketBuffers> = StaticCell::new();
    let buffers = BUFFERS.init(SocketBuffers::default());
    let socket = stack.new_udp_socket(0, buffers);
    log::info!("outbound: endpoint {:?}", socket.endpoint());

    let mut remote_endpoint = None;

    loop {
        match INBOX.receive().await {
            Message::Info { info } => {
                if let Some(endpoint) = remote_endpoint {
                    encode_and_send(&socket, info, endpoint).await;
                }
            }
            Message::Endpoint { endpoint } => {
                remote_endpoint = Some(endpoint);
            }
        }
    }
}

async fn encode_and_send(socket: &UdpSocket<'_>, info: Info, endpoint: IpEndpoint) {
    let mut buf = [0; 256];
    let Ok(bytes) = to_slice(&info, &mut buf) else {
        log::info!("outbound: encoding");
        return;
    };

    if let Err(error) = socket.send_to(bytes, endpoint).await {
        log::info!("outbound: error: {:?}", error);
    };
}
