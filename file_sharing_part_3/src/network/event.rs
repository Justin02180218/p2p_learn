use std::{
    collections::{HashMap, HashSet},
    error::Error,
};

use futures::{io, StreamExt};
use libp2p::{
    core::either::EitherError,
    kad::{GetProvidersOk, KademliaEvent, QueryId, QueryResult},
    multiaddr::Protocol,
    request_response::{RequestId, RequestResponseEvent, RequestResponseMessage, ResponseChannel},
    swarm::{ConnectionHandlerUpgrErr, SwarmEvent},
    PeerId, Swarm,
};
use tokio::sync::{mpsc, oneshot};

use crate::client::Command;

use super::{
    behaviour::{ComposedBehaviour, ComposedEvent},
    protocol::{FileRequest, FileResponse},
};

#[derive(Debug)]
pub enum Event {
    InboundRequest {
        request: String,
        channel: ResponseChannel<FileResponse>,
    },
}

// 事件处理
pub struct EventLoop {
    // P2P网络管理组件
    swarm: Swarm<ComposedBehaviour>,
    // 命令通道接收端
    command_receiver: mpsc::Receiver<Command>,
    // 事件通道发送端
    event_sender: mpsc::Sender<Event>,
    // 缓存等待链接节点的请求
    pending_dial: HashMap<PeerId, oneshot::Sender<Result<(), Box<dyn Error + Send>>>>,
    // 缓存节点提供共享文件的请求
    pending_start_providing: HashMap<QueryId, oneshot::Sender<()>>,
    // 缓存获取提供共享文件节点的请求
    pending_get_providers: HashMap<QueryId, oneshot::Sender<HashSet<PeerId>>>,
    // 缓存获取共享文件内容的请求
    pending_request_file:
        HashMap<RequestId, oneshot::Sender<Result<String, Box<dyn Error + Send>>>>,
}

impl EventLoop {
    pub fn new(
        swarm: Swarm<ComposedBehaviour>,
        command_receiver: mpsc::Receiver<Command>,
        event_sender: mpsc::Sender<Event>,
    ) -> Self {
        Self {
            swarm,
            command_receiver,
            event_sender,
            pending_dial: Default::default(),
            pending_start_providing: Default::default(),
            pending_get_providers: Default::default(),
            pending_request_file: Default::default(),
        }
    }

    pub async fn run(mut self) {
        // 异步轮询事件
        loop {
            tokio::select! {
                event = self.swarm.next() => self.handle_event(event.expect("Swarm stream to be infinite.")).await,
                command = self.command_receiver.recv() => match command {
                    Some(c) => self.handle_command(c).await,
                    None=>  return,
                },
            }
        }
    }

    // 异步处理网络行为事件
    async fn handle_event(
        &mut self,
        event: SwarmEvent<ComposedEvent,EitherError<ConnectionHandlerUpgrErr<io::Error>, io::Error>>,
    ) {
        match event {
            // 节点提供共享文件事件
            SwarmEvent::Behaviour(ComposedEvent::Kademlia(
                KademliaEvent::OutboundQueryCompleted {
                    id,
                    result: QueryResult::StartProviding(_),
                    ..
                },
            )) => {
                // 从缓存中节点提供共享文件的请求
                let sender: oneshot::Sender<()> = self
                    .pending_start_providing
                    .remove(&id)
                    .expect("Completed query to be previously pending.");

                // 发送命令执行成功状态    
                let _ = sender.send(());
            }
            // 获取提供共享文件的节点事件
            SwarmEvent::Behaviour(ComposedEvent::Kademlia(
                KademliaEvent::OutboundQueryCompleted {
                    id,
                    result: QueryResult::GetProviders(Ok(GetProvidersOk { providers, .. })),
                    ..
                },
            )) => {
                // 从缓存中删除获取提供共享文件节点的请求，并发送提供的节点
                let _ = self
                    .pending_get_providers
                    .remove(&id)
                    .expect("Completed query to be previously pending.")
                    .send(providers);
            }
            SwarmEvent::Behaviour(ComposedEvent::Kademlia(_)) => {}
            // 请求文件内容事件
            SwarmEvent::Behaviour(ComposedEvent::RequestResponse(
                RequestResponseEvent::Message { message, .. },
            )) => match message {
                RequestResponseMessage::Request {
                    request, channel, ..
                } => {
                    self.event_sender
                        .send(Event::InboundRequest {
                            request: request.0,
                            channel,
                        })
                        .await
                        .expect("Event receiver not to be dropped.");
                }
                RequestResponseMessage::Response {
                    request_id,
                    response,
                } => {
                    let _ = self
                        .pending_request_file
                        .remove(&request_id)
                        .expect("Request to still be pending.")
                        .send(Ok(response.0));
                }
            },
            SwarmEvent::Behaviour(ComposedEvent::RequestResponse(
                RequestResponseEvent::OutboundFailure {
                    request_id, error, ..
                },
            )) => {
                let _ = self
                    .pending_request_file
                    .remove(&request_id)
                    .expect("Request to still be pending.")
                    .send(Err(Box::new(error)));
            }
            SwarmEvent::Behaviour(ComposedEvent::RequestResponse(
                RequestResponseEvent::ResponseSent { .. },
            )) => {}
            // 本地监听事件
            SwarmEvent::NewListenAddr { address, .. } => {
                let local_peer_id = *self.swarm.local_peer_id();
                println!(
                    "Local node is listening on {:?}",
                    address.with(Protocol::P2p(local_peer_id.into()))
                );
            }
            SwarmEvent::IncomingConnection { .. } => {}
            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => {
                if endpoint.is_dialer() {
                    if let Some(sender) = self.pending_dial.remove(&peer_id) {
                        let _ = sender.send(Ok(()));
                    }
                }
            }
            SwarmEvent::ConnectionClosed { .. } => {}
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(peer_id) = peer_id {
                    if let Some(sender) = self.pending_dial.remove(&peer_id) {
                        let _ = sender.send(Err(Box::new(error)));
                    }
                }
            }
            SwarmEvent::IncomingConnectionError { .. } => {}
            SwarmEvent::Dialing(peer_id) => println!("Dialing {}", peer_id),
            e => panic!("{:?}", e),
        }
    }

    // 异步处理命令事件
    async fn handle_command(&mut self, command: Command) {
        match command {
            // 监听本地节点
            Command::StartListening { addr, sender } => {
                let _ = match self.swarm.listen_on(addr) {
                    Ok(_) => sender.send(Ok(())),
                    Err(e) => sender.send(Err(Box::new(e))),
                };
            }
            // 节点加入KAD网络，链接指定节点，插入缓存
            Command::Dial {
                peer_id,
                peer_addr,
                sender,
            } => {
                if self.pending_dial.contains_key(&peer_id) {
                    todo!("Already dialing peer.");
                } else {
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, peer_addr.clone());
                    match self
                        .swarm
                        .dial(peer_addr.with(Protocol::P2p(peer_id.into())))
                    {
                        Ok(()) => {
                            self.pending_dial.insert(peer_id, sender);
                        }
                        Err(e) => {
                            let _ = sender.send(Err(Box::new(e)));
                        }
                    }
                }
            }
            // 节点提供共享文件，插入缓存
            Command::StartProviding { file_name, sender } => {
                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .start_providing(file_name.into_bytes().into())
                    .expect("No store error.");
                self.pending_start_providing.insert(query_id, sender);
            }
            // 获取提供共享文件的节点，插入缓存
            Command::GetProviders { file_name, sender } => {
                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .get_providers(file_name.into_bytes().into());
                self.pending_get_providers.insert(query_id, sender);
            }
            // 请求共享文件，插入缓存
            Command::RequestFile {
                file_name,
                peer,
                sender,
            } => {
                let request_id = self
                    .swarm
                    .behaviour_mut()
                    .request_response
                    .send_request(&peer, FileRequest(file_name));
                self.pending_request_file.insert(request_id, sender);
            }
            // 返回共享文件内容
            Command::RespondFile { file, channel } => {
                self.swarm
                    .behaviour_mut()
                    .request_response
                    .send_response(channel, FileResponse(file))
                    .expect("Connection to peer to be still open.");
            }
        }
    }
}
