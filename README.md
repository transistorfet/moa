
Moa
===

###### *Started September 26, 2021*

An emulator for m68k CPUs and devices.  I originally started this project to
distract myself while recovering from a bout of sickness.  The idea was to
emulate the computer I had built as part of the
[computie project](https://transistorfet.github.io/projects/computie).

Currently it can run the monitor program and load the kernel across serial
(or the kernel can be loaded directly into memory), and it can boot the kernel.
It opens two PTYs: one for the serial terminal, and one for the SLIP connection,
and launches both `pyserial-miniterm` automatically connected to the console PTY,
and launches `slattach` with the associated setup commands to create the SLIP
device on the host, and set up routing.

There are currently peripheral emulators for the MC68681 dual serial port
controller, and the ATA device, which loads the compact flash image on startup,
which the OS can read.

