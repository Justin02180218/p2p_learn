use std::error::Error;

use args::{CliArgument, Opt};
use clap::Parser;
use client::Client;
use futures::FutureExt;
use libp2p::{multiaddr::Protocol, PeerId};
use tokio::sync::mpsc;

mod args;
mod client;

use client::Command;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let opt = Opt::parse();

    let (command_sender, mut command_receiver) = mpsc::channel(1);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                command = command_receiver.recv() => match command {
                    Some(cmd) => match cmd {
                        Command::StartListening { addr, sender } => {
                            println!("{addr}");
                            sender.send(Ok(())).unwrap();
                        },
                        Command::Dial {peer_id, peer_addr, sender} => {
                            println!("{peer_addr}/{peer_id}");
                            sender.send(Ok(())).unwrap();
                        },
                        Command::StartProviding { file_name, sender } => {
                            println!("{file_name}");
                            sender.send(()).unwrap();
                        },
                        Command::GetProviders { file_name, sender } => {
                            println!("{file_name}");
                            sender.send(Default::default()).unwrap();
                        },
                        Command::RequestFile {file_name, peer, sender} => {
                            println!("{file_name} {peer}");
                            sender.send(Ok("".to_string())).unwrap();
                        },
                        Command::RespondFile { file, channel } => {
                            println!("{file} {:?}", channel);
                        }
                    },
                    None => println!(""),
                }
            }
        }
    });

    let client = Client::new(command_sender);
    process_args(opt, client).await?;

    Ok(())
}

// 解析命令行参数
async fn process_args(opt: Opt, mut client: Client) -> Result<(), Box<dyn Error>> {
    match opt.listen_address {
        Some(addr) => client
            .start_listening(addr)
            .await
            .expect("Listening not to fail."),

        None => client
            .start_listening("/ip4/0.0.0.0/tcp/0".parse()?)
            .await
            .expect("Listening not to fail."),
    }

    if let Some(addr) = opt.peer {
        let peer_id = match addr.iter().last() {
            Some(Protocol::P2p(hash)) => PeerId::from_multihash(hash).expect("Valid hash."),
            _ => return Err("Expect peer multiaddr to contain peer ID.".into()),
        };
        client.dial(peer_id, addr).await.expect("Dial to succeed");
    }

    match opt.argument {
        CliArgument::Provide { path, name } => {
            client.start_providing(name.clone()).await;

            println!("{:?}", path);
        }
        CliArgument::Get { name } => {
            // 找到提供该文件的所有节点
            let providers = client.get_providers(name.clone()).await;
            if providers.is_empty() {
                return Err(format!("Could not find provider for file {}.", name).into());
            }

            // 从每个节点请求文件的内容。
            let requests = providers.into_iter().map(|p| {
                let mut network_client = client.clone();
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
