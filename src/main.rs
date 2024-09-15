#![allow(unused_mut, unused_imports, unused_assignments, unused_parens, invalid_value, non_snake_case, non_upper_case_globals)]

use async_std::{
    io::ReadExt, net::{TcpListener, TcpStream, ToSocketAddrs}, prelude::*, task // 3
};
use joyboot::JoybootClient;
use num_derive::{FromPrimitive, ToPrimitive};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};

mod dolphin;
mod JOY;
mod joyboot;

use dolphin::*;
use JOY::{JOYListener, JOYManager, JOYState};

const localhost: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);

#[async_std::main]
async fn main() -> std::io::Result<()> {
    println!("Connecting to localhost dolphin...");
    let mut conn = DolphinConnection::try_connect(localhost).await.unwrap();
    println!("Done.");
    conn.consumer = Some(JOYManager::new(JoybootClient::new()));
    conn.connection_loop().await;
    return Ok(());
}