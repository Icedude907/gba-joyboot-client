# GBA JoyBus Multiboot Receiver
There are many sources detailing how to _send_ a multiboot program to a GBA via Serial/UART mode - which works in chunks of 16 bits with a fixed baud rate.
However, there doesn't seem to be any project detailing how to _receive_ a program over JoyBus - an alternative protocol which operates in chunks of 32 bits and doesn't require strict timing due to hardware assistance (and can therefore service clients without locking up a CPU core of the sender).

Additionally, this project implements functionality to connect to Dolphin over the network, as if it was a client instance of mGBA.
NOTE: Timing synchronisation is not implemented for the moment.
NOTE: This protocol is *super high overhead* so its likely not useful outside of localhost scenarios.

Progress:
- [x] Connecting to Dolphin via the mGBA protocol
- [x] Handshaking
- [x] Receiving rom
- [x] Deobfuscating
    - TODO: Reading slightly too many bytes.
- [ ] Finalising / Ending transmission
- [ ] Verifying received data
- [ ] Dumping output
- [ ] Ironing out quirks
- [ ] Code cleanup

Sources/Thanks:
- Ghidra
- mGBA
- GBATek
- Shinyquagsire's [gba-multiboot-dump](https://github.com/shinyquagsire23/gba-multiboot-dump) (a SIO receiver).