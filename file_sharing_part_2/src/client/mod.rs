pub mod command;

use std::{collections::HashSet, error::Error};

use libp2p::{request_response::ResponseChannel, Multiaddr, PeerId};
use tokio::sync::{
    mpsc::{self, Sender},
    oneshot,
};

pub use self::command::Command;

// 用于发送命令的Client
#[derive(Clone)]
pub struct Client {
    // 将命令发送到mpsc通道
    sender: mpsc::Sender<Command>,
}

impl Client {
    pub fn new(sender: Sender<Command>) -> Client {
        Client { sender }
    }

    pub async fn start_listening(&mut self, addr: Multiaddr) -> Result<(), Box<dyn Error + Send>> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::StartListening { addr, sender })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not to be dropped.")
    }

    pub async fn dial(
        &mut self,
        peer_id: PeerId,
        peer_addr: Multiaddr,
    ) -> Result<(), Box<dyn Error + Send>> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::Dial {
                peer_id,
                peer_addr,
                sender,
            })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not to be dropped.")
    }

    pub async fn start_providing(&mut self, file_name: String) {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::StartProviding { file_name, sender })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not to be dropped.");
    }

    pub async fn get_providers(&mut self, file_name: String) -> HashSet<PeerId> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::GetProviders { file_name, sender })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not to be dropped.")
    }

    pub async fn request_file(
        &mut self,
        peer: PeerId,
        file_name: String,
    ) -> Result<String, Box<dyn Error + Send>> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::RequestFile {
                file_name,
                peer,
                sender,
            })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not be dropped.")
    }

    #[allow(dead_code)]
    pub async fn respond_file(&mut self, file: String, channel: ResponseChannel<String>) {
        self.sender
            .send(Command::RespondFile { file, channel })
            .await
            .expect("Command receiver not to be dropped.");
    }
}
