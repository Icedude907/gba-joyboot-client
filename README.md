# GBA JoyBus Multiboot Receiver
There are many sources detailing how to _send_ a multiboot program to a GBA via Serial/UART mode - which (to my knowledge) works in chunks of 16 bits with a fixed baud rate.  
However, there doesn't seem to be any project detailing how to _receive_ a program over JoyBus - an alternative protocol which operates in chunks of 32 bits and doesn't require strict serial port timings. Therefore, in the case of a GBA, multiple clients can be serviced at once though at a reduced speed.

Additionally, this project implements functionality to connect to Dolphin over the network, as if it was a client instance of mGBA.  
NOTE: This client does not respect Dolphin's timing synchronisation mechanism. This is intended.  
NOTE: The Dolphin<->GBA network protocol is *super high overhead*. It can't be meaningfully used outside of localhost scenarios.

Once the data is received, the program dumps the program to a file in the current working directory called `multibootrom.mbgba`.

Progress:
- [x] Connecting to Dolphin via the mGBA protocol
- [x] Handshaking
- [x] Receiving rom
- [x] Deobfuscating
- [x] Gracefully ending transmission
- [ ] Verifying received data
- [x] Dumping output
- [ ] Ironing out quirks
- [ ] Code cleanup

Sources/Thanks:
- Ghidra
- mGBA
- GBATek
- Shinyquagsire's [gba-multiboot-dump](https://github.com/shinyquagsire23/gba-multiboot-dump) (a SIO receiver).