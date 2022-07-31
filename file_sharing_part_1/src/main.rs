use std::error::Error;

use args::{Opt, CliArgument};
use clap::Parser;

mod args;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let opt = Opt::parse();

    process_args(opt).await?;

    Ok(())
}

// 解析命令行参数
async fn process_args(opt: Opt) -> Result<(), Box<dyn Error>> {
    match opt.listen_address {
        Some(addr) => println!("{addr}"),
        None => println!("/ip4/0.0.0.0/tcp/0"),
    }

    if let Some(addr) = opt.peer {
        println!("{addr}");
    }

    match opt.argument {
        CliArgument::Provide{ path, name} => {
            println!("{:?}, {name}", path);
        },
        CliArgument::Get { name } => {
            println!("{name}");
        }
    }

    Ok(())
}