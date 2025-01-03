# TODO
- Battletoads 2 open bus
- Get rid of refcell?

[x] CPU COMPLETED
- Better operand handling?
- Open bus behaviour

[x] Implement Memory Bus, Interrupts, synchronization
- Sram saving

[x] Implement PPU
- Cleaner implementation (it is better now but work can still be done)

[x] Mappers
- Implementation is solid, but could be better (and faster)
- Banks should be an array, and change in bank selects changes the bank array
- Mmc5

[x] APU
- Cleaner implementation (it is better now but work can still be done)
- Audio filters?

[x] Support for zip files
[] Consider making the core lib no_std
[] Game info fetcher from online db ??

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
- https://www.nesdev.org/wiki/Tricky-to-emulate_games