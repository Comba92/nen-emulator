## TODO
- [] Famicon Disk System support

- [x] CPU COMPLETED
- [ ] Better operand handling?
- [ ] Getting rid of the instructions json
- Open bus behaviour?

- [x] MMU COMPLETED

- [x] PPU COMPLETED
- Cleaner implementation (it is better now but work can still be done)

- [x] Mappers
- [ ] Convert usizes to u8
- [x] Nametable mirroring can be optimized with the bankings method

- https://www.nesdev.org/wiki/INES_Mapper_210 -- Mapper19 like

- https://www.nesdev.org/wiki/INES_Mapper_037 -- MMC3 like multicart
- https://www.nesdev.org/wiki/INES_Mapper_048 -- MMC3 like
- https://www.nesdev.org/wiki/INES_Mapper_068
- https://www.nesdev.org/wiki/INES_Mapper_091
- [x] https://www.nesdev.org/wiki/INES_Mapper_206 -- MMC3 like 

- https://www.nesdev.org/wiki/INES_Mapper_016
- https://www.nesdev.org/wiki/Sunsoft_FME-7
- https://www.nesdev.org/wiki/INES_Mapper_019
- https://www.nesdev.org/wiki/MMC5

- [x] APU
- Cleaner implementation (it is better now but work can still be done)

- [x] Support for zip files
- [x] Find a fast serializer which WORKS out of the box (it was a problem of buffering!)

- [] Game DB ??

## Tricky games
- [x] MMC1 consecutive reads (Bill & Ted's Excellent Adventure and some other MMC1 games)
- [x] PPUDATA access during rendering (Burai Fighter (U))
- [x] CHR ROM write should have no effect
- [x] Controller open bus (Captain Planet, Dirty Harry, Infiltrator, Mad Max, Paperboy, The Last Starfighter)
- [ ] RAM random content to seed RNG