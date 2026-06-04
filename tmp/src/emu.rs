use std::path::{Path, PathBuf};

use crate::{
    NesPalette,
    apu::{self, ApuRP2A},
    bus::Bus,
    cpu::{self, Cpu6502},
    joypad::Joypad,
    mapper::{self, BoxedMapper, Mapper},
    ppu::Ppu2C02,
    rom::{Cart, Disk, RomData},
    utils::RingBuffer,
};

#[derive(Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Settings {
    // TODO: not implemented
    pub random_ram: bool,
    pub no_sprite_limit: bool,

    // TODO: not implemented
    pub disable_background: bool,
    // TODO: not implemented
    pub disable_sprites: bool,
    // TODO: not implemented
    pub pal_borders: bool,

    // TODO: not implemented
    pub audio_sample_rate: usize,
    pub volume: f32,
    pub disable_pulse0: bool,
    pub disable_pulse1: bool,
    pub disable_triangle: bool,
    pub disable_noise: bool,
    pub disable_dmc: bool,
    pub disable_ext_audio: bool,
}

impl Settings {
    pub fn new() -> Self {
        Self {
            no_sprite_limit: true,
            audio_sample_rate: 44100,
            volume: 0.5,
            ..Default::default()
        }
    }
}

pub const BIOS_CRC32: u32 = 1583381967;
pub const BATTERY_SAVE_EXTENSION: &str = "srm";

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NesEmulator {
    pub cpu: Cpu6502,
    pub ppu: Ppu2C02,
    pub apu: ApuRP2A,
    pub joypad: Joypad,
    pub mem: Bus,
    pub mapper: Box<dyn Mapper>,

    pub frame_ready: bool,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub(crate) videobuf: [u8; 256 * 240 * 4],
    #[cfg_attr(feature = "serde", serde(skip))]
    pub(crate) audiobuf: RingBuffer<f32>,

    pub palette: NesPalette,
    pub settings: Settings,
}

#[derive(Debug, Default, Clone, PartialEq, bitcode::Encode, bitcode::Decode)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Mirroring {
    #[default]
    Horizontal,
    Vertical,
    LowTable,
    HighTable,
    FourScreens,
}

pub const NTSC_CLOCK_RATE: usize = 1789773;
pub const PAL_CLOCK_RATE: usize = 1662607;

pub const NTSC_FRAME_RATE: f32 = 60.0988;
pub const PAL_FRAME_RATE: f32 = 50.0070;

#[derive(Debug, Default, Clone, bitcode::Encode, bitcode::Decode)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Region {
    #[default]
    NTSC,
    PAL,
}
impl Region {
    pub fn clock_rate(&self) -> usize {
        match self {
            Self::NTSC => NTSC_CLOCK_RATE,
            Self::PAL => PAL_CLOCK_RATE,
        }
    }

    pub fn frame_rate(&self) -> f32 {
        match self {
            Self::NTSC => NTSC_FRAME_RATE,
            Self::PAL => PAL_FRAME_RATE,
        }
    }
}

pub(crate) type LoadError = Box<dyn std::error::Error>;

enum Game {
    Cart(Cart),
    Disk(Disk),
}

impl Game {
    pub fn from(bytes: &[u8]) -> Result<Self, LoadError> {
        if RomData::is_valid_ines(bytes) {
            Ok(Game::Cart(Cart::from(bytes)?))
        } else if Disk::is_valid_fds(bytes) {
            Ok(Game::Disk(Disk::from(bytes)?))
        } else {
            // might be headless rom
            Ok(Game::Cart(Cart::from(bytes)?))
        }
    }
}

impl NesEmulator {
    pub fn empty() -> Self {
        Self {
            cpu: Cpu6502::new(),
            ppu: Ppu2C02::default(),
            apu: ApuRP2A::default(),
            joypad: Joypad::default(),
            mem: Bus::default(),
            mapper: Box::new(mapper::NROM),

            videobuf: [0; _],
            audiobuf: RingBuffer::new(0),
            palette: NesPalette::default(),

            frame_ready: false,
            settings: Settings::default(),
        }
    }

    fn new(game: Game, bios: Option<&[u8]>) -> Result<Self, LoadError> {
        let (mem, mapper) = match game {
            Game::Cart(cart) => {
                let mut mem = Bus::with_cart(cart);
                let mapper: BoxedMapper = mapper::new(&mut mem)?;
                (mem, mapper)
            }
            Game::Disk(disk) => {
                let bios = bios.ok_or("no BIOS ROM provided")?;

                if crc32fast::hash(bios) != BIOS_CRC32 {
                    return Err("not a valid BIOS rom".into());
                }
                Bus::with_disk(disk, bios)
            }
        };

        let palette = NesPalette::from_pal_file(include_bytes!("../utils/2C02G_wiki.pal")).unwrap();

        let frame_rate = mem.header.region.frame_rate();
        let mut emu = Self {
            cpu: Cpu6502::new(),
            ppu: Ppu2C02::new(&mem.header.region),
            apu: ApuRP2A::new(&mem.header.region),
            joypad: Joypad::default(),
            mem,
            mapper,

            videobuf: [255; _],
            audiobuf: RingBuffer::new(
                (4.0 * (apu::AvgResampler::DEFAULT_RESAMPLE_FREQ as f32 / frame_rate)) as usize,
            ),
            palette,

            frame_ready: false,
            settings: Settings::new(),
        };

        emu.cpu.pc = emu.cpu_read16(cpu::InterruptVector::Rst as u16);
        Ok(emu)
    }

    pub fn load_rom_from_bytes(rom: &[u8], bios: Option<&[u8]>) -> Result<Self, LoadError> {
        let game = Game::from(rom)?;
        Self::new(game, bios)
    }

    pub fn load_bios_only(bios: Option<&[u8]>) -> Result<Self, LoadError> {
        let empty_disk = Disk::default();
        let game = Game::Disk(empty_disk);
        Self::new(game, bios)
    }

    pub const fn region(&self) -> &Region {
        &self.mem.header.region
    }

    pub const fn header(&self) -> &RomData {
        &self.mem.header
    }

    pub fn cpu_tick(&mut self) {
        self.cpu.cycles = self.cpu.cycles.wrapping_add(1);

        self.apu_step();
        self.mapper.step(&mut self.mem, self.cpu.cycles);

        self.ppu_step();
        self.ppu_step();
        self.ppu_step();

        // PAL systems additionally run 3.2 PPU cycles per CPU cycle
        // meaning, every 5 CPU cycles there is an additional PPU cycle
        match self.region() {
            Region::PAL => {
                if self.cpu.cycles % 5 == 0 {
                    self.ppu_step();
                }
            }
            _ => {}
        }
    }

    pub fn step_until_frame_ready(&mut self) {
        while self.mem.nmi {
            self.cpu_step();
        }

        while !self.mem.nmi {
            self.cpu_step();
        }
    }

    pub fn step_until_samples_or_frame_ready(&mut self, samples_amt: usize) {
        self.frame_ready = false;

        while self.audio_queued() < samples_amt && !self.frame_ready {
            self.cpu_step();
        }
    }

    pub fn emu_reset(&mut self) {
        self.cpu_reset();
        self.ppu.reset();
        self.apu.reset();

        // TODO: should reload wram!
        // TODO: some mappers need to be reset too
    }

    pub fn save_battery(&self) -> Option<&[u8]> {
        if self.mem.header.has_battery {
            if self.mem.header.mapper == 1 && self.mem.wram.len() == 16 * 1024 {
                // https://www.nesdev.org/wiki/MMC1#SxROM_board_types
                // Even if the SOROM and SZROM boards utilizes a battery, it is connected to only one PRG-RAM chip. The first RAM chip will not retain its data, but the second one will.
                return Some(&self.mem.wram[8 * 1024..]);
            }

            if self.mem.header.mapper == 5 && self.mem.wram.len() == 16 * 1024 {
                // https://www.nesdev.org/wiki/MMC5#Other_PRG-RAM_notes
                // Games with 16K PRG-RAM only battery-save the first 8K.
                return Some(&self.mem.wram[..8 * 1024]);
            }

            Some(&self.mem.wram)
        } else {
            None
        }
    }

    pub fn load_battery(&mut self, bytes: &[u8]) -> Result<(), LoadError> {
        // TODO: handle MMC1 and MMC5 weird cases

        if !self.header().has_battery {
            return Ok(());
        } else if bytes.len() != self.mem.wram.len() {
            return Err("invalid save ram dump provided, size don't match".into());
        } else {
            self.mem.wram.copy_from_slice(bytes);
            Ok(())
        }
    }

    pub fn is_frame_ready(&self) -> bool {
        self.frame_ready
    }

    pub fn get_video_rgba(&self) -> &[u8; 256 * 240 * 4] {
        // for (i, color) in self
        //     .videobuf
        //     .iter()
        //     .map(|byte| self.palette.0[*byte as usize])
        //     .enumerate()
        // {
        //     buf[i * 4 + 0] = color.0;
        //     buf[i * 4 + 1] = color.1;
        //     buf[i * 4 + 2] = color.2;
        //     buf[i * 4 + 3] = 255;
        // }

        &self.videobuf
    }

    pub fn get_nametables_rgba(&mut self, buf: &mut [u8]) {
        let pttrntbl = self.ppu.ctrl.bg_pttrntbl_addr;

        for table in 0..4 {
            let region_y = if table <= 1 { 0 } else { 256 * 240 * 4 * 2 };
            let region_x = if table == 0 || table == 2 { 0 } else { 256 * 4 };

            for i in 0usize..960 {
                let nametbl_addr = 0x2000 + (table * 1024) + i;
                let tile_id = self.ppu_debug_read(nametbl_addr as u16);
                let pttrn_addr = pttrntbl + ((tile_id as u16) << 4);
                let attr_addr = (0x23c0 + (table * 1024)) | (((i / 32) / 4) << 3) | ((i % 32) / 4);

                let mut attr = self.ppu_debug_read(attr_addr as u16);
                if (i / 32) & 0x2 > 0 {
                    attr >>= 4;
                }
                if (i % 32) & 0x2 > 0 {
                    attr >>= 2;
                }
                attr &= 0x3;

                for row in 0..8 {
                    let pttrn_lo = self.ppu_debug_read(pttrn_addr + row).reverse_bits();
                    let pttrn_hi = self.ppu_debug_read(pttrn_addr + row + 8).reverse_bits();

                    for col in 0..8 {
                        let pixel = (((pttrn_hi >> col) & 1) << 1) | ((pttrn_lo >> col) & 1);
                        let pixel_color = self.ppu.palettes_read((attr * 4 + pixel) as u16);
                        let color = self.palette.0[pixel_color as usize];

                        // row is 256 * 4 * 2 bytes long
                        let y = region_y + (256 * 4 * 2 * ((i / 32) * 8 + row as usize));
                        // pixel is 4 bytes long
                        let x = region_x + (4 * ((i % 32) * 8 + col as usize));

                        buf[y + x + 0] = color.0;
                        buf[y + x + 1] = color.1;
                        buf[y + x + 2] = color.2;
                        buf[y + x + 3] = 255;
                    }
                }
            }
        }
    }

    pub fn audio_queued(&self) -> usize {
        self.audiobuf.queued()
    }

    pub fn get_audio_f32(&mut self, amount: usize) -> (&[f32], Option<&[f32]>) {
        self.audiobuf.take_available_contiguos(amount)
    }

    pub fn load_rom_from_file<P: AsRef<Path>>(
        rom_path: P,
        bios: Option<&[u8]>,
    ) -> Result<Self, LoadError> {
        use std::{
            fs,
            io::{Read, Seek},
        };

        // let mut bytes = Vec::new();
        // let file = fs::File::open(rom_path)?;
        // let mut reader = io::BufReader::new(file);
        // reader.read_to_end(&mut bytes)?;

        let mut file = fs::File::open(rom_path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;

        let res = NesEmulator::load_rom_from_bytes(&bytes, bios);
        match res {
            Ok(_) => res,
            Err(_) => {
                file.rewind()?;
                bytes.clear();

                if let Ok(mut archive) = zip::read::ZipArchive::new(&mut file) {
                    // it is a zip file
                    let mut zip = archive.by_index(0)?;
                    zip.read_to_end(&mut bytes)?;
                    NesEmulator::load_rom_from_bytes(&bytes, bios)
                } else {
                    // not a zip file either
                    res
                }
            }
        }
    }

    pub fn save_battery_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<bool> {
        use std::{fs, io::Write};

        if let Some(sram) = self.save_battery() {
            let mut save_path = PathBuf::from(path.as_ref());
            save_path.set_extension(BATTERY_SAVE_EXTENSION);

            let mut file = fs::File::create(&save_path)?;
            // let mut reader = std::io::BufWriter::new(file);
            // reader.write_all(sram)?;
            file.write_all(sram)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn load_battery_from_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), LoadError> {
        if !self.header().has_battery {
            return Ok(());
        }
        use std::{fs, io::Read};

        let mut load_path = PathBuf::from(path.as_ref());
        load_path.set_extension(BATTERY_SAVE_EXTENSION);

        if let Ok(mut file) = fs::File::open(&load_path) {
            let mut buf = Vec::new();
            // let mut reader = io::BufReader::new(file);
            // reader.read_to_end(&mut buf)?;
            file.read_to_end(&mut buf)?;
            self.load_battery(&buf)
        } else {
            Err("no sram dump file found".into())
        }
    }

    #[cfg(feature = "serde")]
    pub fn savestate<P: AsRef<Path>>(&self, path: P) -> Result<(), LoadError> {
        let mut file = std::fs::File::create(path)?;
        pot::to_writer(self, file).map_err(|e| e.into())
        // let writer = std::io::BufWriter::new(file);
        // pot::to_writer(self, writer).map_err(|e| e.into())
    }

    #[cfg(feature = "serde")]
    pub fn loadstate<P: AsRef<Path>>(&mut self, path: P) -> Result<(), LoadError> {
        let file = std::fs::File::open(path)?;
        // let reader = std::io::BufReader::new(file);
        // let mut new_emu: Emu = pot::from_reader(reader)?;
        let mut new_emu: NesEmulator = pot::from_reader(file)?;

        std::mem::swap(&mut self.mem.prg, &mut new_emu.mem.prg);
        std::mem::swap(&mut self.audiobuf, &mut new_emu.audiobuf);
        std::mem::swap(&mut self.videobuf, &mut new_emu.videobuf);
        std::mem::swap(&mut self.apu.blip, &mut new_emu.apu.blip);
        *self = new_emu;

        Ok(())
    }
}
