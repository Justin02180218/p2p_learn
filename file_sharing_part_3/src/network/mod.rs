pub mod behaviour;
pub mod event;
pub mod protocol;

use std::{error::Error, iter};

use libp2p::{identity::{ed25519, self}, swarm::SwarmBuilder, kad::{Kademlia, store::MemoryStore}, request_response::{RequestResponse, ProtocolSupport}};
pub use protocol::*;
use tokio::sync::mpsc::{Receiver, self};

use crate::client::Client;

use self::{event::{EventLoop, Event}, behaviour::ComposedBehaviour};

pub async fn new(
    secret_key_seed: Option<u8>,
) -> Result<(Client, Receiver<Event>, EventLoop), Box<dyn Error>> {
    // 创建密钥对
    let id_keys = match secret_key_seed {
        Some(seed) => {
            let mut bytes = [0u8; 32];
            bytes[0] = seed;
            let secret_key = ed25519::SecretKey::from_bytes(&mut bytes).expect(
                "this returns `Err` only if the length is wrong; the length is correct; qed",
            );
            identity::Keypair::Ed25519(secret_key.into())
        }
        None => identity::Keypair::generate_ed25519(),
    };
    // 根据公钥生成节点ID
    let peer_id = id_keys.public().to_peer_id();

    // 构建网络层管理组件Swarm
    let swarm = SwarmBuilder::new(
        libp2p::development_transport(id_keys).await?,
        ComposedBehaviour {
            kademlia: Kademlia::new(peer_id, MemoryStore::new(peer_id)),
            request_response: RequestResponse::new(
                FileExchangeCodec(),
                iter::once((FileExchangeProtocol(), ProtocolSupport::Full)),
                Default::default(),
            ),
        },
        peer_id,
    )
    .build();

    let (command_sender, command_receiver) = mpsc::channel(1);
    let (event_sender, event_receiver) = mpsc::channel(1);

    Ok((
        Client::new(command_sender),
        event_receiver,
        EventLoop::new(swarm, command_receiver, event_sender),
    ))
}