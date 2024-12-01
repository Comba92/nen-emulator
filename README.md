# TODO
[x] CPU COMPLETED
- Better cycles counting?
- Better operand handling?

[x] Implement Memory Bus, Interrupts, synchronization
- Sram should be a mirrored vec
- Fix all memory problems (some games expect more memory than provided, or start with the wrong mapper configuration)
(games like Bubble Bobble, TMNT 2 and Batman crash at startup for this reason)

[x] Implement PPU
- PPU masking
- Rendering toggling delay (battletoads)

[x] Mappers
- THERE ARE SOME BUGS WITH MMC1 and MMC3, some games go out of memory!
- Mmc3 ok, irq not implemented
- Mmc2 not working

[] APU if i have the energy
- It's buggy
- PAL support?

[] Support for NES 2.0 format
- missing correct prg and chr sizes parsing

[] Consider making the core lib no_std
[] Game info fetcher from online db


# Resources
- https://www.nesdev.org/NESDoc.pdf

## Cpu
- https://bugzmanov.github.io/nes_ebook/chapter_3.html
- http://www.6502.org/users/obelisk/6502
- https://www.pagetable.com/c64ref/6502
- https://www.masswerk.at/6502/6502_instruction_set.html
###
Illegal Opcodes
- https://www.nesdev.org/undocumented_opcodes.txt
Correct XAA
- http://www.ffd2.com/fridge/docs/6502-NMOS.extra.opcodes
- https://www.nesdev.org/the%20%27B%27%20flag%20&%20BRK%20instruction.txt

## Memory
- https://en.wikibooks.org/wiki/NES_Programming/Memory_Map
- https://emudev.de/nes-emulator/cartridge-loading-pattern-tables-and-ppu-registers/

## Ppu
- https://bugzmanov.github.io/nes_ebook/chapter_6.html
- https://austinmorlan.com/posts/nes_rendering_overview/
- https://leeteng.com/blog/content/writing-nes-emulator
- https://emudev.de/nes-emulator/palettes-attribute-tables-and-sprites/
- https://www.youtube.com/watch?v=-THeUXqR3zY&list=PLrOv9FMX8xJHqMvSGB_9G9nZZ_4IgteYf&index=5&pp=iAQB

## Mappers
- https://mapper.nes.science/
- https://bumbershootsoft.wordpress.com/2022/10/22/nes-beyond-40kb/

## APU
- https://emudev.de/nes-emulator/charming-sound-the-apu/

### Tests
- https://www.nesdev.org/wiki/Emulator_tests
Nestest:
- https://github.com/PyAndy/Py3NES/issues/1