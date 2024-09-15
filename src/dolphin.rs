use async_std::{
    io::ReadExt, net::{TcpListener, TcpStream, ToSocketAddrs}, prelude::*, task // 3
};
use num_derive::{FromPrimitive, ToPrimitive};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};

use crate::JOY::{JOYListener, JOYManager, JOYCMD};

pub struct DolphinConnection<T: JOYListener>{
    pub dat: TcpStream,
    pub clk: TcpStream,
    state: ConnectionState,
    pub consumer: Option<JOYManager<T>>,
}
#[derive(PartialEq, Eq)]
enum ConnectionState{
    WaitFirstClock = 0,
    WaitClock = 1,
    WaitCommand = 2,
}
impl<T: JOYListener> DolphinConnection<T>{
    const VIDEO_TOTAL_LENGTH: i32 = 280896;
    // Number of GBA cycles per bit on the wire (115200 bps) - mGBA
    const CYCLES_PER_BIT: u32 = (0x1000000 / 115200);

    pub async fn try_connect(target: impl Into<IpAddr>) -> Option<Self> {
        let into = target.into();
        let dat = TcpStream::connect(SocketAddr::new(into, 0xd6ba)).await; // "dolphin gba"
        if dat.is_err() { return None; }
        let mut dat = dat.unwrap();
        let _ = dat.set_nodelay(true).inspect_err(|_| println!("Could not set nodelay. This is gonna lag."));
        let clk = TcpStream::connect(SocketAddr::new(into, 0xc10c)).await; // "clock signal"
        if clk.is_err() { return None; }
        let mut clk = clk.unwrap();
        let _ = clk.set_nodelay(true).inspect_err(|_| println!("Could not set nodelay. This is gonna lag."));

        Self::recvflush(&mut dat).await;
        Self::recvflush(&mut clk).await;
        return Some(Self{
            dat, clk,
            state: ConnectionState::WaitFirstClock,
            consumer: None
        });
    }
    pub async fn connection_loop(&mut self){
        let mut clock_slice: i32 = 0;
        loop{
            if self.state == ConnectionState::WaitFirstClock{
                clock_slice = 0;
                self.state = ConnectionState::WaitClock; // Fallthru
            }
            if self.state == ConnectionState::WaitClock{
                if clock_slice < 0 {
                    // TODO: Falling behind? Poll
                }
                let mut slice_recv: [u8; 4] = unsafe{ std::mem::MaybeUninit::uninit().assume_init() };
                self.clk.read_exact(&mut slice_recv).await.unwrap();
                let offset = i32::from_be_bytes(slice_recv);
                clock_slice = clock_slice.wrapping_add(offset);
                // print!("Offset {:8} cycles. ", offset);
                self.state = ConnectionState::WaitCommand; // Fallthru
            }
            if self.state == ConnectionState::WaitCommand{
                if clock_slice < -Self::VIDEO_TOTAL_LENGTH * 4 {
                    // TODO: Falling behind? Poll
                }
                if self.process_command().await {
                    self.state = ConnectionState::WaitClock;
                }
            }
        }
    }
    /// Empty the received buffers to make sure we don't start forwarding garbage.
    async fn recvflush(strm: &mut TcpStream){
        let mut buf: [u8; 32] = unsafe{ std::mem::MaybeUninit::uninit().assume_init() };
        while strm.read(&mut buf).await.unwrap() == size_of_val(&buf) {}
    }
    async fn process_command(&mut self) -> bool{
        let mut bitsOnLine = 8; // This does not include the stop bits due to compatibility reasons
        let mut control_code: [u8; 1] = [0];
        self.dat.read_exact(&mut control_code).await.unwrap();

        // Analyze request
        let control_code = control_code[0];
        let x = num::FromPrimitive::from_u8(control_code);
        if x.is_none(){
            println!("Unexpected JOY Code: {:?}. Bits: {}. Skipping", control_code, bitsOnLine);
            return false;
        }
        let x = x.unwrap();
        let joystat_default = 0;
        match x {
            JOYCMD::JOY_RESET => {
                bitsOnLine += 24;
                let out = self.consumer.as_mut().map_or([0, 4, joystat_default], |x| x.reset());
                self.dat.write(&out).await.unwrap();
            }
            JOYCMD::JOY_POLL  => {
                bitsOnLine += 24;
                let out = self.consumer.as_mut().map_or([0, 4, joystat_default], |x| x.poll());
                self.dat.write(&out).await.unwrap();
            }
            JOYCMD::JOY_TRANS => {
                bitsOnLine += 40;
                let out = self.consumer.as_mut().map_or([0, 0, 0, 0, joystat_default], |x| x.send());
                self.dat.write_all(&out).await.unwrap();
            }
            JOYCMD::JOY_RECV  => {
                bitsOnLine += 40;
                let mut buf: [u8; 4] = unsafe{ std::mem::MaybeUninit::uninit().assume_init() };
                self.dat.read_exact(&mut buf).await.unwrap(); // Read received bytes
                let out = self.consumer.as_mut().map_or([joystat_default], |x| x.recv(buf));
                self.dat.write(&out).await.unwrap();
            }
        }
        self.dat.flush().await.unwrap(); // Send response over the wire.
        return true;
        // bitsOnLine * CYCLES_PER_BIT - cyclesLate
    }
}