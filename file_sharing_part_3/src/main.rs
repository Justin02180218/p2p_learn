use std::error::Error;

use args::{CliArgument, Opt};
use clap::Parser;
use client::Client;
use futures::FutureExt;
use libp2p::{multiaddr::Protocol, PeerId};
use network::event::Event;
use tokio::sync::mpsc::Receiver;

mod args;
mod client;
mod network;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let opt = Opt::parse();

    let (network_client, network_events, network_event_loop) =
        network::new(opt.secret_key_seed).await?;

    tokio::spawn(async move {
        network_event_loop.run().await;
    });

    process_args(opt, network_client, network_events).await?;

    Ok(())
}

// 解析命令行参数
async fn process_args(opt: Opt, mut network_client: Client, mut network_events: Receiver<Event>) -> Result<(), Box<dyn Error>> {
    match opt.listen_address {
        Some(addr) => network_client
            .start_listening(addr)
            .await
            .expect("Listening not to fail."),
        None => network_client
            .start_listening("/ip4/0.0.0.0/tcp/0".parse()?)
            .await
            .expect("Listening not to fail."),
    };

    if let Some(addr) = opt.peer {
        let peer_id = match addr.iter().last() {
            Some(Protocol::P2p(hash)) => PeerId::from_multihash(hash).expect("Valid hash."),
            _ => return Err("Expect peer multiaddr to contain peer ID.".into()),
        };
        network_client
            .dial(peer_id, addr)
            .await
            .expect("Dial to succeed");
    }

    match opt.argument {
        CliArgument::Provide { path, name } => {
            // Advertise oneself as a provider of the file on the DHT.
            network_client.start_providing(name.clone()).await;

            loop {
                match network_events.recv().await {
                    // Reply with the content of the file on incoming requests.
                    Some(Event::InboundRequest { request, channel }) => {
                        if request == name {
                            let file_content = std::fs::read_to_string(&path)?;
                            network_client.respond_file(file_content, channel).await;
                        }
                    }
                    e => todo!("{:?}", e),
                }
            }
        }
        
        CliArgument::Get { name } => {
            // 找到提供该文件的所有节点
            let providers = network_client.get_providers(name.clone()).await;
            if providers.is_empty() {
                return Err(format!("Could not find provider for file {}.", name).into());
            }

            // 从每个节点请求文件的内容。
            let requests = providers.into_iter().map(|p| {
                let mut network_client = network_client.clone();
                let name = name.clone();
                async move { network_client.request_file(p, name).await }.boxed()
            });

            // 等待请求，一旦有一个请求成功，就忽略剩下的请求。
            let file = futures::future::select_ok(requests)
                .await
                .map_err(|_| "None of the providers returned file.")?
                .0;

            println!("Content of file {}: {}", name, file);
        }
    }

    Ok(())
}
