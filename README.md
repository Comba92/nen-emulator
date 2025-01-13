# Nen Emulator
## A simple and clean™️ cycle accurate NES emulator, written in Rust. Fast, lightweight, and with high compatibility.

## Download
Download is avaible in the [release](https://github.com/Comba92/nen-emulator/releases/tag/alpha) section.
The emulator frontend is built with SDL2, with very basic (sadly) user functionality.

## Usage
Game ROMs can be loaded by dragging and dropping the files into the window.
Zip files are supported.
ROMS with iNes or NES2.0 headers are supported.

The terminal window shows basic informations and warnings, such as the ROM information.

### Controls
Both keyboards and controllers are supported.
A save/load state feature is avaible.

<kbd>A</kbd> A button<br>
<kbd>S</kbd> B button<br>
<kbd>W</kbd> Start<br>
<kbd>E</kbd> Select<br>
<kbd>ArrowKeys</kbd> You know what they do!<br>

<kbd>Space</kbd>Pause/unpause the emulator<br>
<kbd>R</kbd>Reset current game<br>
<kbd>M</kbd>Mute/unmute sound<br>
<kbd>9</kbd>Save state<br>
<kbd>0</kbd>Load state<br>

## Compatibility
The emulator supports mostly all the basic NES features you'd expect from a NES emulator.
- [x] The emulator is cycle accurate. A list of tests coverage is avaible [here](https://github.com/Comba92/nen-emulator/blob/master/tests/TESTS_COVERAGE.md).

- [x] The PPU is emulated as a pixel renderer, and emulates the LoopyRegister behaviour. This means it draws pixels to a framebuffer in the same way it draws in a CRT.
- [x] The pixel fethcer is emulated only for the backrounds. Object sprites are all fetched in one go, then mixed with the backround pixels one by one.
- [x] The APU is fully emulated.

- [x] PAL games and speeds are supported.
- [x] Games with tricky and obscure behaviour run correctly, except for one or two exceptions. For more information: https://www.nesdev.org/wiki/Tricky-to-emulate_games
- [x] BATTLETOADS & BATTLETOADS 2 RUN!
- [x] MMC3 four screen mirroring

- [x] iNes and NES2.0 headers are supported.
- [x] Saving/loading of battery RAM when the game is changed or the emulator is closed.
- [x] Savestates
- [ ] Headerless games are not supported.

### Supported Mappers
#### The most popular
- [x] 00. [NROM](https://www.nesdev.org/wiki/NROM)
- [x] 01. [MMC1](https://www.nesdev.org/wiki/MMC1)
- [x] 01. [SxROM variants](https://www.nesdev.org/wiki/MMC1#SxROM_connection_variants)
- [x] 02. [UxROM](https://www.nesdev.org/wiki/UxROM)
- [x] 03. [CNROM](https://www.nesdev.org/wiki/INES_Mapper_003)
- [x] 04. [MMC3](https://www.nesdev.org/wiki/MMC3)
- [x] 04. [MMC6 variant](https://www.nesdev.org/wiki/MMC3#iNES_Mapper_004_and_MMC6)
- [x] 07. [AxROM](https://www.nesdev.org/wiki/AxROM)
- [x] 66. [GxROM](https://www.nesdev.org/wiki/GxROM)

#### Other mappers
- [ ] 05. [MMC5 (TODO)](https://www.nesdev.org/wiki/MMC5)
- [x] 09. [MMC2 (used for Punch-Out!!)](https://www.nesdev.org/wiki/MMC2)
- [x] 10. [MMC4](https://www.nesdev.org/wiki/MMC4)
- [x] 11. [ColorDreams](https://www.nesdev.org/wiki/Color_Dreams)
- [ ] 19. [Namco 129/163 (TODO)](https://www.nesdev.org/wiki/INES_Mapper_019)
- Note: no audio chip emulation.
- [x] 21, 22, 23, 25. [VRC2 and VRC4](https://www.nesdev.org/wiki/VRC2_and_VRC4)
- Note: compatibility might not be the best. (TODO: use submappers to discriminate board)
- [x] 24. [VRC6a](https://www.nesdev.org/wiki/VRC6)
- [x] 26. [VRC6b](https://www.nesdev.org/wiki/VRC6)
- [x] 31. [NSF](https://www.nesdev.org/wiki/INES_Mapper_031)
- [ ] 68. [Sunsoft4 (TODO)](https://www.nesdev.org/wiki/INES_Mapper_068)
- [x] 69. [Sunsoft5 FME-7](https://www.nesdev.org/wiki/Sunsoft_FME-7)
- Note: Batman: Return of Joker works, but doesn't start unless you press Start.
- Note: no audio chip emulation.
- [x] 71. [Codemasters](https://www.nesdev.org/wiki/INES_Mapper_071)
- [x] 73. [VRC3 (used for Salamander)](https://www.nesdev.org/wiki/VRC3)
- [x] 75. [VRC1](https://www.nesdev.org/wiki/VRC1)
- [x] 78. [Irem 74HC161 (used for Holy Diver and Cosmo Carrier)](https://www.nesdev.org/wiki/INES_Mapper_078)
- [ ] 91. [J.Y. Company (TODO)](https://www.nesdev.org/wiki/INES_Mapper_091)
- [x] 180. [UNROM (used for Crazy Climber)](https://www.nesdev.org/wiki/INES_Mapper_180)
- [x] 206. [Namco 118/Tengen MIMIC-1](https://www.nesdev.org/wiki/INES_Mapper_206)
- [ ] 210. [Namco 175 (TODO)](https://www.nesdev.org/wiki/INES_Mapper_210)

## Building
The emulator is served as a stand-alone Rust library. It provides a basic API in `src/nes.rs`, which can be used by any frontend. (TODO: move the Nes struct to `lib.rs`)

Building requires, of course, Rust and it's development tools.
To build the emulator library, simply use in the root folder:
```bash
cargo build -r
```

Two frontends are avaible.
A SDL2 frontend, in frontend-native.
To build, again, it's simply:
```bash
cargo build -r                   # Dynamically linked with SDL2
cargo build --features="static"  # Statically linked with SDL2
```

A WASM frontend is still WIP.

### Architecture
#### Dependency tree
The emulator is organized as a tree (sort of) of dependencies. The root is the CPU. which contains the BUS, and which in turn contains all the peripherals:
- PPU
- DMA
- APU
- Cartridge/Mapper
- Joypads

The cartridge is the only exception, as it is shared with multiple peripherals.
Rust's typing system makes it hard to create circular dependencies of pointers, so the Cartride object has to be wrapped in a RefCell for interior mutability (giving us the ability to mutate it anywhere), and then in a Rc, for shared ownership.

This approach proved to be solid, safe, and reliable, but at the cost of flexibility.
The architecure had to be changed from its roots multiple times, as most of the features and roadblocks weren't took into account since the beginning. This is the heart of software development though. You can never plan in advance how the system can be designed efficently.

An alternative solution would be having a global "god struct" (as the folks at [emudev discord](https://discordapp.com/invite/dkmJAes) like to call), which contains a general emulation context, which is passed to every function. This increases flexibility, as there are fewer restrictions, but with a less cohesive design.

As the NES is a relatively simple machine, restricting the design to a dependency tree proved to be beneficial. Everything is tightly coupled, and the codebase is manageable.
Adding more user features is a pain, tho.

#### TODO: detailed explanation of the architecture. ;)

### What's missing
- [ ] MMC1 consecutive writes behaviour
- [ ] RAM initializing for games which uses it to seed RNG
- [ ] MMC5

- [ ] Custom keybindings
- [ ] Custom palettes
- [ ] Sprite limit per scanline
- [ ] Headerless ROMs support

## Various resources
This section contains some of the resources I've used during development. Sadly I didn't keep track of all of them. I did found a lot of interesting articles, blogs, and readings, but forgot to add them here.

- https://www.nesdev.org/wiki
- https://www.nesdev.org/NESDoc.pdf
- https://www.copetti.org/writings/consoles/nes/

### CPU
- https://www.nesdev.org/6502.txt
- https://www.nesdev.org/6502_cpu.txt
- https://bugzmanov.github.io/nes_ebook/chapter_3.html
- http://www.6502.org/users/obelisk/6502
- https://www.pagetable.com/c64ref/6502
- https://www.masswerk.at/6502/6502_instruction_set.html
#### Illegal Opcodes
- https://www.nesdev.org/undocumented_opcodes.txt
#### Correct XAA
- http://www.ffd2.com/fridge/docs/6502-NMOS.extra.opcodes
- https://www.nesdev.org/the%20%27B%27%20flag%20&%20BRK%20instruction.txt

### Memory
- https://en.wikibooks.org/wiki/NES_Programming/Memory_Map
- https://www.nesdev.org/wiki/CPU_memory_map
- https://www.nesdev.org/wiki/PPU_memory_map
- https://emudev.de/nes-emulator/cartridge-loading-pattern-tables-and-ppu-registers/

### Ppu
- https://bugzmanov.github.io/nes_ebook/chapter_6.html
- https://austinmorlan.com/posts/nes_rendering_overview/
- https://leeteng.com/blog/content/writing-nes-emulator
- https://emudev.de/nes-emulator/palettes-attribute-tables-and-sprites/
- https://www.youtube.com/watch?v=-THeUXqR3zY

### Mappers
- https://mapper.nes.science/
- https://bumbershootsoft.wordpress.com/2022/10/22/nes-beyond-40kb/

### APU
- https://emudev.de/nes-emulator/charming-sound-the-apu/
- https://www.slack.net/~ant/nes-emu/apu_ref.txt
- https://nesmaker.nerdboard.nl/2022/03/25/the-apu/

### Tests
- https://www.nesdev.org/wiki/Emulator_tests
- https://www.nesdev.org/wiki/Tricky-to-emulate_games
- https://github.com/SingleStepTests/ProcessorTests/tree/main/nes6502
- https://github.com/PyAndy/Py3NES/issues/1
