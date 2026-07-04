# Nen Emulator
## NES emulator, written in Rust. High mappers compatibility, FDS support. 

![](https://github.com/user-attachments/assets/5a431cf1-2d2a-4f96-abb0-3cae2c4c6ebd)

## Download
Download is avaible in the [releases](https://github.com/Comba92/nen-emulator/releases/) section.
An eframe frontend with [egui](https://github.com/emilk/egui) is available, with a GUI and a lot of user functionality.
A [SDL2 and SDL3](https://www.libsdl.org/) frontend for the emulator is avaible, with very basic user functionality.
A (WIP) WASM frontend is also avaible here: https://comba92.github.io/nen-emulator/frontend-wasm/index.html

## Usage (eframe frontend)
Game ROMs can be loaded by dragging and dropping the files into the window, or by using the file dialog in the top bar.
> [!TIP]
> Zip files are supported.

### Controls
> [!TIP]
> Both keyboards and controllers are supported.
> [!TIP]
> Controls can be changed in the keybids menu.

| Keyboard | Button |
| :--: | ----- |
| <kbd>S</kbd> | A button |
| <kbd>A</kbd> | B button |
| <kbd>W</kbd> | Start |
| <kbd>E</kbd> | Select |
| <kbd>ArrowKeys</kbd> | You know what they do! |
| <kbd>P</kbd> | Pause/unpause the emulator |
| <kbd>R</kbd> | Reset current game |
| <kbd>M</kbd> | Mute/unmute sound |
| <kbd>0</kbd> | Save state |
| <kbd>9</kbd> | Load state |

## Features
The emulator supports all the basic NES features you'd expect from a NES emulator.
- [x] The [6502 CPU](https://www.nesdev.org/wiki/CPU) is emulated with most of its quirks.
- [x] PPU pixel rendering emulation. Emulates the [LoopyRegister behaviour](https://www.nesdev.org/wiki/PPU_scrolling#PPU_internal_registers) and the [pixel fethcer](https://www.nesdev.org/wiki/PPU_rendering).
- [x] The [APU](https://www.nesdev.org/wiki/APU) channels are all fully emulated.
- [x] Both NTSC and PAL games are supported.
- [x] The [Famicom Disk System](https://www.nesdev.org/wiki/Family_Computer_Disk_System) is supported.
- [x] Custom color [palettes](https://www.nesdev.org/wiki/PPU_palettes) are supported
- [x] Games with [tricky and obscure behaviour](https://www.nesdev.org/wiki/Tricky-to-emulate_games) run correctly, except for one or two exceptions.
- [x] RAM random initializing for games which uses it to seed RNG
- [x] Saving/loading of battery RAM when the game is changed or the emulator is closed.
- [x] Savestates
- [x] Setting to disable the sprites limit for scanline.

## Compatibility
- [iNes](https://www.nesdev.org/wiki/INES) and [NES2.0](https://www.nesdev.org/wiki/NES_2.0) headers are supported.
- Headerless ROMs are supported thanks to a [games database](https://forums.nesdev.org/viewtopic.php?t=19940).
- [FDS](https://www.nesdev.org/wiki/FDS_disk_format) disks roms for the Famicom Disk System are supported. 
- Zip files are supported.

### Supported Mappers
#### The most popular
- [x] 00. [NROM](https://www.nesdev.org/wiki/NROM)
- [x] 01. [MMC1](https://www.nesdev.org/wiki/MMC1)
- [x] 02. [UxROM](https://www.nesdev.org/wiki/UxROM)
- [x] 03. [CNROM](https://www.nesdev.org/wiki/INES_Mapper_003)
- [x] 04. [MMC3](https://www.nesdev.org/wiki/MMC3)
- [x] 07. [AxROM](https://www.nesdev.org/wiki/AxROM)
- [x] 66. [GxROM](https://www.nesdev.org/wiki/GxROM)

#### Other mappers
- [x] 01. [MMC1 SxROM variants](https://www.nesdev.org/wiki/MMC1#SxROM_connection_variants)
- [x] 04. [MMC6 (used for Startropics and Startropics II)](https://www.nesdev.org/wiki/MMC6)
- [x] 05. [MMC5](https://www.nesdev.org/wiki/MMC5)
  - Note: PCM channel not implemented (only one game seems to be using it)
  - Note: Vertical split functionality not implemented (almost no game seems to use it, or use it in very obscure parts like rare cutscenes)
- [x] 09. [MMC2 (used for Punch-Out!!)](https://www.nesdev.org/wiki/MMC2)
- [x] 10. [MMC4](https://www.nesdev.org/wiki/MMC4)
- [x] 11. [ColorDreams](https://www.nesdev.org/wiki/Color_Dreams)
- [x] 13. [CPROM (only used for Videomation, a painting program for NES)](https://www.nesdev.org/wiki/CPROM)
- [ ] 16, [Bandai FCG](https://www.nesdev.org/wiki/INES_Mapper_016)
  - Note: Something is implemented but not usable, graphical glitches
- [x] 19. [Namcot 129/163](https://www.nesdev.org/wiki/INES_Mapper_019)
- [x] 20 [Famicom Disk System](https://www.nesdev.org/wiki/INES_Mapper_020) 
- [x] 21, 22, 23, 25. [VRC2 and VRC4](https://www.nesdev.org/wiki/VRC2_and_VRC4)
- [x] 24. [VRC6a (only used for Akumajou Densetsu - Japanese version of Castlevania III with enhanced audio)](https://www.nesdev.org/wiki/VRC6)
- [x] 26. [VRC6b (used for Madara and Esper Dream 2)](https://www.nesdev.org/wiki/VRC6)
- [x] 29 [Various Homebrews](https://www.nesdev.org/wiki/INES_Mapper_029) 
- [ ] 30. [UNROM512](https://www.nesdev.org/wiki/UNROM_512)
  - Note: flashable board PRG not implemented
- [x] 31. [NSF (music compilations roms)](https://www.nesdev.org/wiki/INES_Mapper_031)
- [x] 34. [BNROM/NINA-001](https://www.nesdev.org/wiki/INES_Mapper_034)
- [x] 40. [NTDEC 27x2](https://www.nesdev.org/wiki/INES_Mapper_040)
- [x] 67. [Sunsoft-3](https://www.nesdev.org/wiki/INES_Mapper_067) 
- [x] 68. [Sunsoft-4](https://www.nesdev.org/wiki/INES_Mapper_068)
  - Note: Nantettatte!! Baseball (J) licensing IC not implemented
- [x] 69. [Sunsoft5 FME-7 / Sunsoft 5a/5b](https://www.nesdev.org/wiki/Sunsoft_FME-7)
- [x] 70. [Bandai-74](https://www.nesdev.org/wiki/INES_Mapper_070) 
- [x] 71. [Codemasters](https://www.nesdev.org/wiki/INES_Mapper_071)
- [x] 73. [VRC3 (used for Salamander)](https://www.nesdev.org/wiki/VRC3)
- [x] 75. [VRC1](https://www.nesdev.org/wiki/VRC1)
- [x] 76. [Namcot-3446 (only used for Megami Tensei: Digital Devil Story)](https://www.nesdev.org/wiki/INES_Mapper_076) 
- [x] 77. [Napoleon Senki](https://www.nesdev.org/wiki/INES_Mapper_077)
- [x] 78. [Irem 74HC (only used for Holy Diver and Cosmo Carrier)](https://www.nesdev.org/wiki/INES_Mapper_078)
- [x] 79. [NINA-003-006](https://www.nesdev.org/wiki/NINA-003-006) 
- [x] 85. [VRC7 (used for Lagrange Point and Tiny Toon Adventures 2 (J))](https://www.nesdev.org/wiki/VRC7)
  - Note: audio chip not implemented (only used by Lagrange Point)
- [x] 87. [J87](https://www.nesdev.org/wiki/INES_Mapper_087)
- [x] 88. [Namcot 118](https://www.nesdev.org/wiki/INES_Mapper_088)
- [x] 89. [Sunsoft89 (only used by Tenka no Goikenban: Mito Koumon)](https://www.nesdev.org/wiki/INES_Mapper_089) 
- [x] 93. [Sunsoft93](https://www.nesdev.org/wiki/INES_Mapper_093) 
- [x] 94. [UNROM (only used for Senjou no Ookami - Japanese version of Commando)](https://www.nesdev.org/wiki/INES_Mapper_094)
- [x] 95. [Namcot-3425 (only used for Dragon Buster (J))](https://www.nesdev.org/wiki/INES_Mapper_095) 
- [x] 97. [Irem TAM-S1 (only used by Kaiketsu Yanchamaru)](https://www.nesdev.org/wiki/INES_Mapper_097) 
- [x] 101. [J87 (only used for Urusei Yatsura - Lum no Wedding Bell)](https://www.nesdev.org/wiki/INES_Mapper_101)
- [ ] 111. [GTROM](https://www.nesdev.org/wiki/GTROM)
  - Note: flashable board PRG not implemented
- [x] 152. [Bandai/Taito 74](https://www.nesdev.org/wiki/INES_Mapper_152)
- [x] 154. [Namcot-3453 (only used for Devil Man)](https://www.nesdev.org/wiki/INES_Mapper_154) 
- [x] 177. [BNROM Hengedianzi](https://www.nesdev.org/wiki/INES_Mapper_177)
- [x] 180. [UNROM (only used for Crazy Climber)](https://www.nesdev.org/wiki/INES_Mapper_180)
- [x] 184 [Sunsoft-1](https://www.nesdev.org/wiki/INES_Mapper_184) 
- [x] 206. [Namco 118/Tengen MIMIC-1](https://www.nesdev.org/wiki/INES_Mapper_206)
- [x] 210. [Namco 175/340](https://www.nesdev.org/wiki/INES_Mapper_210)
- [x] 232 [Codemasters](https://www.nesdev.org/wiki/INES_Mapper_232) 
- [x] 241. [BxROM Hengedianzi](https://www.nesdev.org/wiki/INES_Mapper_241)  

## Building
Building requires, of course, Rust and it's development tools.

Three frontends are avaible.
The eframe frontend is the default one.
To build it:
```bash
cargo run -r
cargo run -r --features=persistence # enable savestates feature
cargo run -r --features=opengl # use opengl graphical backend for eframe
```

A SDL2 and a SDL3 frontends are also available. They are their own separate package, so the `-p` flag is needed.
To build them:
```bash
cargo build -r -p nenemu_sdl2 # Dynamically linked with SDL2, no savestates feature
cargo build -r -p nenemu_sdl2 --features="static"  # Statically linked with SDL2
```

The WASM frontend is still WIP, but already avaible here: https://comba92.github.io/nen-emulator/frontend-wasm/index.html

## Various resources (not updated)
This section contains some of the resources I've used during development. Sadly I didn't keep track of all of them. I did found a lot of interesting articles, blogs, and readings, but forgot to add them here.

- https://www.nesdev.org/wiki
- https://www.nesdev.org/NESDoc.pdf
- https://www.copetti.org/writings/consoles/nes/

### Design
- https://gendev.spritesmind.net/forum/viewtopic.php?p=35653&sid=8395d8875e87653545c0905281b093a0#p35653
- https://gist.github.com/adamveld12/d0398717145a2c8dedab
- https://www.nesdev.org/wiki/Catch-up
- https://www.reddit.com/r/EmuDev/comments/10m9had/cpu_ppu_timing_catch_up_method/


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
