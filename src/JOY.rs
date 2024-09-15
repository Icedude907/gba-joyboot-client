use async_std::{
    io::ReadExt, net::{TcpListener, TcpStream, ToSocketAddrs}, prelude::*, task // 3
};
use num_derive::{FromPrimitive, ToPrimitive};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};

#[repr(u8)]
#[derive(PartialEq, Eq, FromPrimitive, Debug)]
#[allow(non_camel_case_types)]
pub enum JOYCMD{
    JOY_RESET = 0xFF,
	JOY_POLL = 0x00,
	JOY_TRANS = 0x14,
	JOY_RECV = 0x15
}

pub struct JOYManager<T: JOYListener>{
    state: JOYState,
    consumer: T
}
impl<T: JOYListener> JOYManager<T>{
    pub fn new(consumer: T) -> Self{
        let mut x = Self{ state: JOYState{ joystat: 0, send_buf: 0, recv_buf: 0}, consumer };
        x.consumer.handle_init(&mut x.state);
        return x;
    }

    /// Processes JOY_RESET
    pub fn reset(&mut self)->[u8; 3]{
        // println!("JOY reset -> stat = {:02x}", self.state.joystat);
        let ret = [0, 4, self.state.joystat];
        self.consumer.handle_reset(&mut self.state);
        return ret;
    }
    /// Processes JOY_POLL
    pub fn poll(&mut self)->[u8; 3]{
        // println!("JOY poll -> stat = {:02x}", self.state.joystat);
        let ret = [0, 4, self.state.joystat];
        self.consumer.on_poll(&mut self.state);
        return ret;
    }
    /// Processes JOY_RECV
    pub fn recv(&mut self, data: [u8; 4]) -> [u8; 1]{
        self.state.joystat |= 0b0000_0010; // Set flag
        let ret = [self.state.joystat];
        // println!("JOY recv: {:02x}{:02x}{:02x}{:02x} -> {:02x}", data[0], data[1], data[2], data[3], ret[0]);
        self.state.recv_buf = u32::from_le_bytes(data); // NOTE: data[0] is the bottom byte of JOY_RECV_LO, hence LE.
        self.consumer.on_recv(&mut self.state);
        return ret;
    }
    /// Processes JOY_TRANS
    pub fn send(&mut self) -> [u8; 5]{
        let ret = {
            let b = self.state.send_buf.to_le_bytes(); // JOY_TRANS_LO is out[0], hence the send buffer is little endian.
            [b[0], b[1], b[2], b[3], self.state.joystat]
        };
        // println!("JOY send -> {:02x}{:02x}{:02x}{:02x}, stat = {:02x}", ret[0], ret[1], ret[2], ret[3], ret[4]);
        self.state.joystat &= 0b1111_0111; // Clear flag (happens after the packet is sent)
        self.consumer.on_send(&mut self.state);
        return ret;
    }
}

pub trait JOYListener{
    /// Triggered when the JOY environment is created. Analogous to turning on.
    fn handle_init(&mut self, context: &mut JOYState){ let _ = context; }
    /// Triggered when receiving the JOY_RESET command.
    fn handle_reset(&mut self, context: &mut JOYState);
    /// Triggered when the previous data was sent out and you can place new data in.
    /// You don't need to service the request.
    fn on_send(&mut self, context: &mut JOYState);
    /// Triggered when there is data for processing in the recv buffer.
    /// You don't need to service the request.
    fn on_recv(&mut self, context: &mut JOYState);

    /// On GBA hardware receiving a poll is entirely hardware processed, and you can't do anything in particular about it.
    /// However, a handler is implemented for debugging.
    fn on_poll(&mut self, context: &mut JOYState){ let _ = context; }
}

pub struct JOYState{
    pub joystat: u8,
    pub send_buf: u32,
    pub recv_buf: u32,
}
impl JOYState{
    /// Returns the data and a flag signalling if it has changed since last read.
    /// Clears the flag, which is visible to the 
    pub fn read_recv_buf(&mut self) -> (u32, bool){
        let changed: bool = (self.joystat & 0b0000_0010) != 0;
        self.joystat &= 0b1111_1101; // Clear flag
        // Yield the data
        return (self.recv_buf, changed);
    }

    /// Update the outgoing data.
    /// Returns 'false' if overwriting a value yet to be sent.
    pub fn write_send_buf(&mut self, dat: u32) -> bool{
        let buf_empty: bool = (self.joystat & 0b0000_1000) == 0;
        self.joystat |= 0b0000_1000; // write sets 0x8
        self.send_buf = dat;
        return buf_empty;
    }

    /// Try update the outgoing data. Does nothing if the previous data has yet to be sent.
    /// Returns 'true' if the outgoing data was set correctly.
    pub fn try_write_send(&mut self, dat: u32) -> bool{
        let buf_empty: bool = (self.joystat & 0b0000_1000) == 0;
        if buf_empty { self.write_send_buf(dat); }
        return buf_empty;
    }

    /// Updates the joystat register (send to the master with every request),
    /// preseving automatically managed bitflags.
    pub fn write_joy_safe(&mut self, x: u8){
        self.joystat = x & 0b1111_0101
          | self.joystat & 0b0000_1010;
    }
}