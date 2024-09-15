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
}
impl JoybootClient{
    // Real clients use pseudo-random value (from doRandom?). This is just one I dumped.
    const CLIENT_KEY: u32 = 0xD4CC95B4;
    // Little endian B4, 95, CC, D4
    // hex(0xD4CC95B4 ^ 0x6f646573)  =>  '0xbba8f0c7'

    // TCRF says this was developer self-credit in the obfuscation bytes, brilliant!
    const KeyMagic: [u8; 8] = *b"Kawasedo";
    const KeyClientTrf: u32 = 0x6f646573; // 'sedo'
    const KeyData: u32 = 0x6177614B; // 'awaK'

    pub fn new() -> Self{
        Self{
            state: JoybootClientState::Init,
            ewram: vec![],
            datalen: 0, readpos: 0,
        }
    }
}
impl JOYListener for JoybootClient{
    fn handle_init(&mut self, context: &mut JOYState) {
        // Tells remote we are alive. Remote sends RESET.
        context.write_send_buf(0);
    }
    fn handle_reset(&mut self, context: &mut JOYState) {
        // Send our 'random' key.
        context.write_send_buf(Self::CLIENT_KEY ^ Self::KeyClientTrf);
        context.write_joy_safe(0x10);
        self.state = JoybootClientState::Init;
    }
    fn on_recv(&mut self, context: &mut JOYState) {
        match self.state{
            JoybootClientState::Init => {
                let data = context.read_recv_buf().0; // E.g. dfc1f5d7

                let header_decrypt = {
                    let magicidx = ((data & 0x200) >> 7) as usize;
                    u32::from_le_bytes(Self::KeyMagic[magicidx..magicidx+4].try_into().unwrap())
                };
                let sessionKey = data ^ header_decrypt;
                println!("\tRecv {:08x} -> Decryption Key {:08x} -> Session Key {:08x}", data, header_decrypt, sessionKey);

                let mut uVar3 = (sessionKey >> 8) & 0x7f;
                if sessionKey & 0x10000 != 0 {
                    uVar3 += 0x80;
                }
                uVar3 = ((uVar3 << 7 | sessionKey & 0x7f) + 0x3f) << 3;
                let mut datalen = 0x0003FFF8 & uVar3; // The protocol implicitly limits the max transfer size to ~256k (size of ewram) - slightly off.
                if (datalen != uVar3) {
                    // It appears this is some kind of error detection mechanism.
                    // Triggering this prevents booting on hardware. (i.e.: bit 23 of KeyB must be 1)
                    // *(byte *)(wram_base_r3 + 10) = keyB
                    // *(byte *)(wram_base_r3 + 10) = *(byte *)(wram_base_r3 + 10) & 0x7f;
                    datalen = 0x4480;
                    println!("Error: Tripped detection mechanism?")
                }

                let mut datalen = datalen + 0xc;
                println!("\tData length: {} bytes", datalen);

                // Next state
                self.datalen = datalen;
                self.ewram = vec![0; datalen as usize / 4 + 4];
                self.readpos = 0;
                self.state = JoybootClientState::RecvHeader;
                context.write_joy_safe(0x20);
            },
            JoybootClientState::RecvHeader => {
                let data = context.read_recv_buf().0;
                context.joystat ^= 0x10; // Flip this bit each time we want another u32

                self.ewram[self.readpos as usize/4] = data;
                self.readpos += 4;
                if(self.readpos >= 0xc0){
                    self.state = JoybootClientState::RecvObfuscated;
                    println!("\tBegin Recv Obfuscated Data...");
                }
            },
            JoybootClientState::RecvObfuscated => {
                let data = context.read_recv_buf().0;
                context.joystat ^= 0x10;
                self.ewram[self.readpos as usize/4] = data;
                self.readpos += 4;

                if(self.readpos >= self.datalen){
                    println!("\tDONE recv.");
                    self.state = JoybootClientState::Dump;
                    self.dodecrypt();
                    println!("DONE decrypt.");
                        // DEBUG: Dump to file
                        let bytes: Vec<u8> = self.ewram.iter().map(|x| x.to_le_bytes()).flatten().collect();
                        std::fs::write("./multibootrom.mbgba", bytes).unwrap();
                        println!("Done write.");
                    std::process::exit(0);
                }
            },
            JoybootClientState::Dump => {
                // TODO: What now?
            }
        }
    }
    fn on_send(&mut self, context: &mut JOYState) {
        // Fires after we send our CLIENT_KEY - makes sure the master knows we sent it.
        context.write_joy_safe(0x10); // 0x12
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
    // Single iteration of the bios random function. Generates the transmission key.
    fn doRandom(x: u32)->u32{
        return x.wrapping_mul(Self::KeyData).wrapping_add(1);
    }

    // Note: GBA bios only does max steps of 137 u16 chunks at a time(?)
    /// One-shot decryption of the whole rom.
    fn dodecrypt(&mut self){
        let mut index: u32 = 0xC0;
        // Rolling key
        let mut key: u32 = Self::CLIENT_KEY;
        const key_type: u32 = 0x20796220; // JOYBUS = 0x20796220, Normal = 0x43202F2F, Multi = 0x6465646F.

        while(index <= self.datalen){
            // Key iteration
            key = key.wrapping_mul(Self::KeyData).wrapping_add(1);
            // println!("KeyTransform: {:08x} * {:08x} + 1 -> {:08x}", key, key_const, key2);
            // Decode
            let ptrkey = (0x02000000 + index).wrapping_neg();
            let word = self.ewram[index as usize /4] ^ key ^ ptrkey ^ key_type;
            // Store
            // println!("Decrypt: {:08x} ^ {:08x} ^ {:08x} ^ {:08x} --> {:08x}", self.ewram[index as usize /4], key, ptrkey, key_type, word);
            self.ewram[index as usize /4] = word;
            // TODO:
            // let CRC = Self::docrc(self.crc, word, primpoly);
            index += 4;
        }
    }
}