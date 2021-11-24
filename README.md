
Moa
===

###### *Started September 26, 2021*

Moa is an emulator/simulator for computers using various 68000 and Z80 CPUs and
peripherals.  The original idea was to emulate the computer I had built as part
of the [Computie project](https://transistorfet.github.io/projects/computie).

Currently it can simulate Computie (68000) and the TRS-80 Model I (Z80), and I'm
working on supporting the Macintosh 512k and Sega Genesis.

For Computie, it can do everything the 68k-SMT board can do, including run the
monitor program and load the Computie OS kernel and boot it from the
CompactFlash card.  To run it:
```
cargo run -p moa-console --bin moa-computie
```
It will open two PTYs: one for the serial terminal, and one for the SLIP
connection.  Once open, it will try to launch both `pyserial-miniterm` as a
separate process and connect to the Computie PTY.  It will also launch
`slattach` with the associated setup commands to create the SLIP device on the
host, and set up host routing.  The exact commands in
`src/machines/computie.rs` might need to be adjusted to work on different
hosts.

For the TRS-80, it can run Level I or Level II Basic, but it doesn't yet
support a cassette tape drive or floppy drive.  I haven't tested it that
thoroughly either, so any help with it would be welcome.  I mostly made it to
test the Z80 cpu implementation in a simpler computer before I used in the
Genesis emulator.  The frontend uses the
[`minifb`](https://github.com/emoon/rust_minifb) rust crate to open a window
and render the characters to screen, as well as accept input from the keyboard.
```
cargo run -p moa-minifb --release --bin moa-trs-80 --
```
By default it will start Level I Basic.  To use the other rom, add the option
`--rom binaries/trs80/level2.rom`

The Genesis emulator is a work in progress but can be run with:
```
cargo run -p moa-minifb --release --bin moa-genesis
```

By default, the minifb frontend will scale the window by 2.  This can be
changed with the `--scale [1,2,4]` option.

The `-d` or `--debugger` option will make the emulator start the debugger
before running.  There is a simple built-in debugger for stepping through
the rom instructions being emulated.  The state of the CPU registers will
be displayed after each instruction, breakpoints can be set, memory contents
can be examined, and memory locations can be modified.  This has helped a lot
with tracking down errors in the emulator itself.

