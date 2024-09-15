//! Multiboot client spoofer over the JoyBus protocol.
use crate::JOY::{JOYListener, JOYState};

enum JoybootClientState{
    Init,
    RecvHeader,
    RecvObfuscated, // Post 0xC0 bytes
    Dump,
}

pub struct JoybootClient{
    state: JoybootClientState,
    ewram: Vec<u32>,
    //
    datalen: u32,
    readpos: u32,

    sessionkey: u32,
}
impl JoybootClient{
    /// Max EWRAM size in 4-byte units.
    const EWRAM_SIZE: usize = 256*1024/4;

    // Real clients use pseudo-random value (from doRandom?). This is just one dumped.
    const CLIENT_KEY: u32 = 0xB495CCD4_u32.swap_bytes();
    // Mem B5B205E4 -> Wire C6D7618B 
    // Looks like JOY_TRANS = Client ^ 0x6f646573

    // Little endian B4, 95, CC, D4
    // hex(0xD4CC95B4 ^ 0x6f646573)  =>  '0xbba8f0c7' (sent over the wire as C7F0A8BB says mgba)

    // TCRF says this was developer self-credit in the obfuscation bytes, brilliant!
    const KeyMagic: [u8; 8] = *b"Kawasedo";

    pub fn new() -> Self{
        Self{
            state: JoybootClientState::Init,
            ewram: vec![0; Self::EWRAM_SIZE],
            datalen: 0, readpos: 0,
            sessionkey: 0,
        }
    }
}
impl JOYListener for JoybootClient{
    fn handle_init(&mut self, context: &mut JOYState) {
        // Tells remote alive. Remote sends RESET.
        context.write_send_buf(0);
    }
    fn handle_reset(&mut self, context: &mut JOYState) {
        // Send our 'random' key.
        context.write_send_buf((Self::CLIENT_KEY ^ 0x6f646573).swap_bytes());
        context.write_joy(0x10);
        self.state = JoybootClientState::Init;
    }
    fn on_recv(&mut self, context: &mut JOYState) {
        match self.state{
            JoybootClientState::Init => {
                let data = context.read_recv_buf().0; // E.g. dfc1f5d7
                let data = data.swap_bytes();         // TODO: Byteorders?

                let idxstart = ((data & 0x200) >> 7) as usize;
                let decryptbytes = u32::from_le_bytes(Self::KeyMagic[idxstart..idxstart+4].try_into().unwrap());
                let sessionKey = data ^ decryptbytes;
                println!("\tRecv {:08x} -> Decryption Key {:08x} -> Session Key {:08x}, IDX {}", data, decryptbytes, sessionKey, idxstart);

                let mut uVar3 = (sessionKey >> 8) & 0x7f;
                if sessionKey & 0x10000 != 0 {
                    uVar3 += 0x80;
                }
                uVar3 = ((uVar3 << 7 | sessionKey & 0x7f) + 0x3f) << 3;
                let mut datalen = 0x0003FFF8 & uVar3;
                if (datalen != uVar3) {
                    // It appears this is some kind of error detection mechanism.
                    // Triggering this prevents booting on hardware. (i.e.: bit 23 of KeyB must be 1)
                    // *(byte *)(wram_base_r3 + 10) = keyB
                    // *(byte *)(wram_base_r3 + 10) = *(byte *)(wram_base_r3 + 10) & 0x7f;
                    datalen = 0x4480;
                }
                let datalen = datalen + 0xc;
                let destptr = 0x0200_0000 + datalen;
                println!("\tFinal pointer: {:08x} (data length: {} bytes)", destptr, datalen);
                // NOTE: More testing required.

                // Next state
                self.datalen = datalen;
                self.sessionkey = sessionKey;
                self.readpos = 0;
                self.state = JoybootClientState::RecvHeader;
                context.write_joy(0x20);
            },
            JoybootClientState::RecvHeader => {
                let data = context.read_recv_buf().0;
                context.joystat ^= 0x10; // Flip this bit each time we want another u32

                // let write = self.datalen as usize;
                // self.ewram[write+0] = (data>>24) as u8;
                // self.ewram[write+1] = (data>>16) as u8;
                // self.ewram[write+2] = (data>> 8) as u8;
                // self.ewram[write+3] = (data) as u8;
                self.ewram[self.readpos as usize/4] = data;
                self.readpos += 4;
                if(self.readpos >= 0xc0){
                    self.state = JoybootClientState::RecvObfuscated;
                    println!("\tBegin Obfuscated Data (after this)...");
                }
            },
            JoybootClientState::RecvObfuscated => {
                let data = context.read_recv_buf().0;
                // Recreation of the bios functions at 0x2fe0 & 0x287a. TODO:
                context.joystat ^= 0x10;
                // let mut targetAddr = 0x0200_0000 + self.readpos;
                    self.ewram[self.readpos as usize/4] = data.swap_bytes();
                    self.readpos += 4;
                    // targetAddr += 4;
                // println!("\t|-> {:08x}", 0);

                if(self.readpos >= 0x50*4 /*self.datalen*/){
                    println!("DONE recv.");
                    self.state = JoybootClientState::Dump;
                    // std::process::exit(1);
                }
            },
            JoybootClientState::Dump => {
                self.dodecrypt();
                // println!("{:08x?}", &self.ewram[0..=0x50]);
                std::process::exit(0);
            }
        }
    }
    fn on_send(&mut self, context: &mut JOYState) {
        context.joystat = 0x12;
    }
}
impl JoybootClient{
    // 0x2864. I'm pretty confident in this.
    fn docrc(crc: u32, src: u32, magic: u32) -> u32{
        let mut crc = crc;
        let mut src = src;
        for _ in 0..32 {
            let temp = crc ^ src;
            crc >>= 1;
            if (temp & 1) != 0 {
                crc ^= magic;
            }
            src >>= 1;
        }
        return crc;
    }
    // Single iteration of the random function.
    fn doRandom(x: u32)->u32{
        return x.wrapping_mul(0x6177614b).wrapping_add(1);
    }
    // fn decryptsingle(&mut self) -> u32{
    //     Self::docrc(crc, src, primpoly);
    //     unimplemented!();
    // }

    /// One-shot decryption of the whole rom. Incomplete. Credit to Shinyquagsire.
    // Note: GBA bios only does max steps of 137 u16 chunks at a time
    fn dodecrypt(&mut self){
        let mut index: u32 = 0xC0;
        // Load Key A
        // uVar2 = (*(uint *)(work_base_reg + 0x4c) & 0xffffff1f ^ 0xa0) & 0xffff7fff;
        // seed = *(0x03000058);

        // Key is the value at 0x03000010 after receiving the first byte.
        // I'm not sure how to properly attain this value. Its obviously related to the value transmitted over the wire.

        // let mut key = (seed & 0xffff_ff1f ^ 0xa0) & 0xffff7fff;
        // let mut key = Self::CLIENT_KEY;
        // let mut key = ((Self::CLIENT_KEY & 0x00ff0000) >> 8) + 0xffff00d1;
        let mut key: u32 = Self::CLIENT_KEY;
        //let mut key: u32 = 0x601AF3A8; //0; // 0xFFFFEA00 | (pp as u32); // TODO?
        const key_const: u32 = 0x6177614B; //0x6F646573; // b"// Coded by Kawasedo" -> 'awaK'
        const key_type: u32 = 0x20796220; // JOYBUS = 0x20796220, Normal = 0x43202F2F, Multi = 0x6465646F

        while(index <= 0xd0){
            // Key iteration
            let key2 = key.wrapping_mul(key_const).wrapping_add(1); // CONFIRMED CORRECT
            // println!("KeyTransform: {:08x} * {:08x} + 1 -> {:08x}", key, key_const, key2);
            key = key2;
            // Decode with key
            let ptrkey = (0x02000000 + index).wrapping_neg();
            let word = self.ewram[index as usize /4] // TODO: That just leaves you.....
                       ^ key // CORRECT
                       ^ ptrkey // CORRECT
                       ^ key_type // CORRECT
                       ;
            // Store
            println!("Decrypt: {:08x} ^ {:08x} ^ {:08x} ^ {:08x} --> {:08x}", self.ewram[index as usize /4], key, ptrkey, key_type, word);
            self.ewram[index as usize /4] = word;
            // println!("Decode - key {:08x}, ", key);
            // TODO: key iteration
            // let CRC = Self::docrc(self.crc, word, primpoly);
            index += 4;
        }

    }
}