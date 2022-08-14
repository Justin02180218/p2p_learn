use std::{collections::HashSet, error::Error};

use libp2p::{request_response::ResponseChannel, Multiaddr, PeerId};
use tokio::sync::oneshot;

use crate::network::FileResponse;

#[derive(Debug)]
pub enum Command {
    // 监听本地端口命令
    StartListening {
        // 本地监听地址
        addr: Multiaddr,
        // 用于发送命令执行状态的通道
        sender: oneshot::Sender<Result<(), Box<dyn Error + Send>>>,
    },
    // 链接给定节点命令
    Dial {
        // 节点ID
        peer_id: PeerId,
        // 节点地址
        peer_addr: Multiaddr,
        // 用于发送命令执行状态的通道
        sender: oneshot::Sender<Result<(), Box<dyn Error + Send>>>,
    },
    // 宣称本节点提供共享文件命令
    StartProviding {
        // 文件名称
        file_name: String,
        // 用于发送命令执行状态的通道
        sender: oneshot::Sender<()>,
    },
    // 获取提供共享文件的节点命令
    GetProviders {
        // 文件名称
        file_name: String,
        // 用于发送命令执行状态的通道
        sender: oneshot::Sender<HashSet<PeerId>>,
    },
    // 请求共享文件命令
    RequestFile {
        // 文件名称
        file_name: String,
        // 节点ID
        peer: PeerId,
        // 用于发送命令执行状态的通道
        sender: oneshot::Sender<Result<String, Box<dyn Error + Send>>>,
    },
    // 返回共享文件内容命令
    RespondFile {
        // 文件名称
        file: String,
        // 返回文件内容
        channel: ResponseChannel<FileResponse>,
    },
}
