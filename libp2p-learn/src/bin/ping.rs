use std::error::Error;

use libp2p::{
    futures::StreamExt,
    identity,
    ping::{Ping, PingConfig},
    swarm::SwarmEvent,
    Multiaddr, PeerId, Swarm,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 生成密钥对
    let key_pair = identity::Keypair::generate_ed25519();

    // 基于密钥对的公钥，生成节点唯一标识peerId
    let peer_id = PeerId::from(key_pair.public());
    println!("节点ID: {peer_id}");

    // 声明Ping网络行为
    let behaviour = Ping::new(PingConfig::new().with_keep_alive(true));

    // 传输
    let transport = libp2p::development_transport(key_pair).await?;

    // 网络管理模块
    let mut swarm = Swarm::new(transport, behaviour, peer_id);

    // 在节点随机开启一个端口监听
    swarm.listen_on("/ip4/127.0.0.1/tcp/0".parse()?)?;

    // 从命令行参数获取远程节点地址，进行链接。
    if let Some(remote_peer) = std::env::args().nth(1) {
        let remote_peer_multiaddr: Multiaddr = remote_peer.parse()?;
        swarm.dial(remote_peer_multiaddr)?;
        println!("链接远程节点: {remote_peer}");
    }

    loop {
        // 匹配网络事件
        match swarm.select_next_some().await {
            // 监听事件
            SwarmEvent::NewListenAddr { address, .. } => {
                println!("本地监听地址: {address}");
            }
            // 网络行为事件
            SwarmEvent::Behaviour(event) => println!("{:?}", event),
            _ => {}
        }
    }
}
