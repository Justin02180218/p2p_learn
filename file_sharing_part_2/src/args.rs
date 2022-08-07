use std::path::PathBuf;

use clap::Parser;
use libp2p::Multiaddr;

#[derive(Debug, Parser)]
#[clap(name = "P2P File Sharing")]
pub struct Opt {
    // 生成密钥对的种子
    #[clap(long)]
    pub secret_key_seed: Option<u8>,

    // 节点地址
    #[clap(long)]
    pub peer: Option<Multiaddr>,

    // 监听地址
    #[clap(long)]
    pub listen_address: Option<Multiaddr>,

    // 子命令
    #[clap(subcommand)]
    pub argument: CliArgument,
}

#[derive(Debug, Parser)]
pub enum CliArgument {
    // 提供文件子命令
    Provide {
        #[clap(long)]
        path: PathBuf, // 文件全路径
        #[clap(long)]
        name: String, // 文件名称
    },
    // 获取文件内容子命令
    Get {
        #[clap(long)]
        name: String, // 文件名称
    },
}
