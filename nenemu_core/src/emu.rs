use std::{
    ops::Not,
    path::{Path, PathBuf},
};

use crate::{
    NesPalette,
    apu::ApuRP2A,
    bus::Bus,
    cpu::{self, Cpu6502},
    joypad::Joypad,
    mapper::{self, Mapper},
    ppu::Ppu2C02,
    rom::{Cart, Disk, RomData},
    utils::{AvgResampler, RingBuffer},
};

#[derive(Default, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
pub enum SampleRate {
    Hz32000 = 32000,
    Hz44100 = 44100,
    #[default]
    Hz48000 = 48000,
    Hz96000 = 96000,
}
impl Into<f32> for SampleRate {
    fn into(self) -> f32 {
        self as u32 as f32
    }
}

#[derive(Default, Clone, PartialEq)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
pub struct NesSettings {
    // TODO: settings should be inverted because we're always negating them
    pub random_ram: bool,
    pub disable_sprite_limit: bool,
    pub enable_oam_read: bool,

    pub disable_background: bool,
    pub disable_sprites: bool,

    // TODO: not implemented
    pub pal_borders: bool,

    // TODO: not sure about this being here
    pub audio_sample_rate: SampleRate,
    pub disable_pulse0: bool,
    pub disable_pulse1: bool,
    pub disable_triangle: bool,
    pub disable_noise: bool,
    pub disable_dmc: bool,
    pub disable_ext_audio: bool,
}

#[derive(Default)]
pub(crate) struct NesOutput {
    pub(crate) frame_ready: bool,
    pub(crate) videobuf_back: Box<Framebuf>,
    pub(crate) videobuf_view: Box<Framebuf>,
    pub(crate) audiobuf: RingBuffer<f32>,
    pub(crate) resamplebuf: Vec<f32>,

    pub resampler: AvgResampler,
}
impl NesOutput {
    pub fn new(region: &Region) -> Self {
        Self {
            audiobuf: RingBuffer::new(
                (AUDIO_FRAMES_BUFFERED as f32 * (region.clock_rate() as f32 / region.frame_rate()))
                    as usize,
            ),
            ..Default::default()
        }
    }
}

pub(crate) struct Framebuf(pub [u8; FRAMEBUF_SIZE]);
impl Default for Framebuf {
    fn default() -> Self {
        Self([255; _])
    }
}

// TODO: not everything should serialized (especially in BUS)
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
pub struct NesEmulator {
    pub cpu: Cpu6502,
    pub ppu: Ppu2C02,
    pub apu: ApuRP2A,
    pub joy: Joypad,
    pub mem: Bus,
    pub mapper: Box<dyn Mapper>,

    #[cfg_attr(feature = "savestates", serde(skip))]
    pub(crate) output: NesOutput,

    pub palette: NesPalette,
    pub settings: NesSettings,
}

#[derive(Debug, Default, Clone, PartialEq, bitcode::Encode, bitcode::Decode)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
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

pub const SCREEN_WIDTH: isize = 256;
pub const SCREEN_HEIGHT: isize = 240;

pub const FRAMEBUF_SIZE: usize = SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize * 4;
pub const AUDIO_FRAMES_BUFFERED: usize = 2;

pub const BIOS_CRC32: u32 = 1583381967;
pub const BATTERY_SAVE_EXTENSION: &str = "srm";

#[derive(Debug, Default, Clone, bitcode::Encode, bitcode::Decode)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
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
    pub fn from<B: AsRef<[u8]>>(bytes: B) -> Result<Self, LoadError> {
        let bytes = bytes.as_ref();

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
        // Self {
        //     cpu: Cpu6502::new(),
        //     ppu: Ppu2C02::default(),
        //     apu: ApuRP2A::default(),
        //     joypad: Joypad::default(),
        //     mem: Bus::with_cart(Cart::default()),
        //     mapper: Box::new(mapper::NROM),

        //     videobuf: Box::new([255; _]),
        //     audiobuf: RingBuffer::new(0),
        //     palette: NesPalette::default(),

        //     // frame_ready: false,
        //     settings: Settings::default(),
        // }

        Self::new(Game::Cart(Cart::default()), None::<&[u8]>).unwrap()
    }

    pub fn debug() -> Self {
        Self {
            cpu: Cpu6502::new(),
            ppu: Ppu2C02::default(),
            apu: ApuRP2A::default(),
            joy: Joypad::default(),
            mem: Bus::with_ram_64kb(),
            mapper: Box::new(mapper::NROM),

            output: NesOutput::default(),
            palette: NesPalette::default(),

            settings: NesSettings::default(),
        }
    }

    fn new_with_settings<B: AsRef<[u8]>>(
        game: Game,
        bios: Option<B>,
        settings: NesSettings,
    ) -> Result<Self, LoadError> {
        let (mem, mapper) = match game {
            Game::Cart(cart) => {
                let mut mem = Bus::with_cart(cart);
                let mapper = mapper::new(&mut mem)?;
                (mem, mapper)
            }
            Game::Disk(disk) => {
                let bios = bios.ok_or("no BIOS ROM provided")?;
                let bios = bios.as_ref();

                if crc32fast::hash(bios) != BIOS_CRC32 {
                    return Err("not a valid BIOS rom".into());
                }
                Bus::with_disk(disk, bios)
            }
        };

        let palette = NesPalette::from_pal_file(include_bytes!("../utils/2C02G_wiki.pal")).unwrap();

        let mut emu = Self {
            output: NesOutput::new(&mem.header.region),

            cpu: Cpu6502::new(),
            ppu: Ppu2C02::new(&mem.header.region),
            apu: ApuRP2A::new(&mem.header.region),
            joy: Joypad::default(),
            mem,
            mapper,

            palette,
            settings,
        };

        if emu.settings.random_ram {
            // Final Fantasy, River City Ransom, Apple Town Story[5], Impossible Mission II[6] amongst others
            // Use the semi-random contents of RAM on powerup to seed their RNGs.
            getrandom::fill(&mut emu.mem.ram)?;
            getrandom::fill(&mut emu.mem.wram)?;
        }

        emu.cpu.pc = emu.cpu_read16(cpu::InterruptVector::Rst as u16);
        Ok(emu)
    }

    fn new<B: AsRef<[u8]>>(game: Game, bios: Option<B>) -> Result<Self, LoadError> {
        Self::new_with_settings(game, bios, NesSettings::default())
    }

    pub fn load_rom_from_bytes<R: AsRef<[u8]>, B: AsRef<[u8]>>(
        rom: R,
        bios: Option<B>,
    ) -> Result<Self, LoadError> {
        let game = Game::from(rom)?;
        Self::new(game, bios)
    }

    pub fn load_bios_only<B: AsRef<[u8]>>(bios: Option<B>) -> Result<Self, LoadError> {
        let empty_disk = Disk::default();
        let game = Game::Disk(empty_disk);
        Self::new(game, bios)
    }

    pub fn region(&self) -> &Region {
        &self.mem.header.region
    }

    pub fn clock_rate(&self) -> usize {
        self.region().clock_rate()
    }

    pub fn frame_rate(&self) -> f32 {
        self.region().frame_rate()
    }

    pub fn rom_info(&self) -> &RomData {
        &self.mem.header
    }

    pub fn update_settings(&mut self, settings: NesSettings) {
        self.settings = settings;
        self.set_audio_rate(self.settings.audio_sample_rate.into());
    }

    pub(crate) fn step_devices(&mut self) {
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

    pub fn step_until_frame_ready(&mut self) -> Result<(), &'static str> {
        while self.is_frame_ready() {
            self.cpu_step();
        }

        while !self.is_frame_ready() {
            self.cpu_step();
        }

        self.cpu
            .jammed
            .not()
            .then(|| ())
            .ok_or("cpu panicked (reached a jam instruction or unimplemented illegal)")
    }

    pub fn is_frame_ready(&self) -> bool {
        self.output.frame_ready
    }

    pub fn step_until_samples_or_frame_ready(
        &mut self,
        samples_amt: usize,
        sample_rate: usize,
    ) -> Result<(), &'static str> {
        while self.audio_queued(sample_rate) < samples_amt && self.is_frame_ready() {
            self.cpu_step();
        }

        while self.audio_queued(sample_rate) < samples_amt && !self.is_frame_ready() {
            self.cpu_step();
        }

        self.cpu
            .jammed
            .not()
            .then(|| ())
            .ok_or("cpu panicked (reached a jam instruction or unimplemented illegal)")
    }

    pub fn emu_reset(&mut self) {
        self.cpu_reset();
        self.ppu.reset();
        self.apu.reset();

        // TODO: should reload wram battery!
        // TODO: some mappers need to be reset too
    }

    pub fn get_video_rgba(&self) -> &[u8; FRAMEBUF_SIZE] {
        &self.output.videobuf_view.0
    }

    pub fn put_video_rgba(&self, buf: &mut [u8]) {
        buf.copy_from_slice(self.get_video_rgba());
    }

    pub fn get_nametables_rgba(&self, buf: &mut [u8]) {
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

    fn resample(&mut self, amt: usize) {
        let output = &mut self.output;

        output.resamplebuf.clear();

        while output.resamplebuf.len() < amt {
            let sample = output.audiobuf.pop();
            if let Some(resample) = output.resampler.add_sample(*sample) {
                output.resamplebuf.push(resample);
            }
        }
    }

    pub fn audio_queued(&self, rate: usize) -> usize {
        // queued : CLOCKRATE = x : TargetRate
        (self.output.audiobuf.queued() as f64 * rate as f64 / self.clock_rate() as f64) as usize
    }

    pub fn set_audio_rate(&mut self, rate: f32) {
        self.output
            .resampler
            .set_rate(self.region().clock_rate(), rate);
    }

    pub fn get_audio_f32(&mut self, amount: usize) -> &[f32] {
        self.resample(amount);
        &self.output.resamplebuf
    }

    pub fn get_audio_f32_all(&mut self, sample_rate: usize) -> &[f32] {
        self.get_audio_f32(self.audio_queued(sample_rate))
    }

    pub fn put_audio_f32(&mut self, buf: &mut [f32]) {
        // let (right, left) = self.get_audio_f32(buf.len());
        // buf[..right.len()].copy_from_slice(right);

        // if let Some(left) = left {
        //     buf[right.len()..].copy_from_slice(left);
        // }
        buf.copy_from_slice(self.get_audio_f32(buf.len()));
    }

    pub fn load_rom_from_file<P: AsRef<Path>, B: AsRef<[u8]>>(
        rom_path: P,
        bios: Option<B>,
    ) -> Result<Self, LoadError> {
        use std::{
            fs,
            io::{Read, Seek},
        };

        let mut bytes = Vec::new();
        let mut file = fs::File::open(rom_path)?;
        let mut reader = std::io::BufReader::new(&file);
        reader.read_to_end(&mut bytes)?;

        // let mut bytes = Vec::new();
        // let mut file = fs::File::open(rom_path)?;
        // file.read_to_end(&mut bytes)?;

        let res = Game::from(&bytes);
        match res {
            Ok(game) => Self::new(game, bios),
            Err(e) => {
                file.rewind()?;
                bytes.clear();

                if let Ok(mut archive) = zip::read::ZipArchive::new(&mut file) {
                    // it is a zip file
                    let mut zip = archive.by_index(0)?;
                    zip.read_to_end(&mut bytes)?;
                    NesEmulator::load_rom_from_bytes(&bytes, bios)
                } else {
                    // not a zip file either
                    Err(e)
                }
            }
        }
    }

    pub fn save_battery(&self) -> Option<&[u8]> {
        if self.rom_info().has_battery {
            if self.rom_info().mapper == 1 && self.mem.wram.len() == 16 * 1024 {
                // https://www.nesdev.org/wiki/MMC1#SxROM_board_types
                // Even if the SOROM and SZROM boards utilizes a battery, it is connected to only one PRG-RAM chip. The first RAM chip will not retain its data, but the second one will.
                return Some(&self.mem.wram[8 * 1024..]);
            }

            if self.rom_info().mapper == 5 && self.mem.wram.len() == 16 * 1024 {
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
        if !self.rom_info().has_battery {
            return Ok(());
        } else if bytes.len() != self.mem.wram.len() {
            return Err("invalid save ram dump provided, size don't match".into());
        } else {
            if self.rom_info().mapper == 1 && self.mem.wram.len() == 16 * 1024 {
                // https://www.nesdev.org/wiki/MMC1#SxROM_board_types
                // Even if the SOROM and SZROM boards utilizes a battery, it is connected to only one PRG-RAM chip. The first RAM chip will not retain its data, but the second one will.
                self.mem.wram[8 * 1024..].copy_from_slice(bytes)
            } else if self.rom_info().mapper == 5 && self.mem.wram.len() == 16 * 1024 {
                // https://www.nesdev.org/wiki/MMC5#Other_PRG-RAM_notes
                // Games with 16K PRG-RAM only battery-save the first 8K.
                self.mem.wram[..8 * 1024].copy_from_slice(bytes)
            } else {
                self.mem.wram.copy_from_slice(bytes);
            }

            Ok(())
        }
    }

    pub fn save_battery_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<bool> {
        use std::{fs, io::Write};

        if let Some(sram) = self.save_battery() {
            let mut save_path = PathBuf::from(path.as_ref());
            save_path.set_extension(BATTERY_SAVE_EXTENSION);

            let file = fs::File::create(&save_path)?;
            let mut reader = std::io::BufWriter::new(file);
            reader.write_all(sram)?;
            // file.write_all(sram)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn load_battery_from_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), LoadError> {
        if !self.rom_info().has_battery {
            return Ok(());
        }
        use std::{fs, io::Read};

        let mut load_path = PathBuf::from(path.as_ref());
        load_path.set_extension(BATTERY_SAVE_EXTENSION);

        if let Ok(file) = fs::File::open(&load_path) {
            let mut buf = Vec::new();
            let mut reader = std::io::BufReader::new(file);
            reader.read_to_end(&mut buf)?;
            // file.read_to_end(&mut buf)?;
            self.load_battery(&buf)
        } else {
            Err("no sram dump file found".into())
        }
    }

    #[cfg(feature = "savestates")]
    pub fn savestate<P: AsRef<Path>>(&self, path: P) -> Result<(), LoadError> {
        let file = std::fs::File::create(path)?;
        // pot::to_writer(self, file).map_err(|e| e.into())
        let writer = std::io::BufWriter::new(file);
        pot::to_writer(self, writer).map_err(|e| e.into())
    }

    #[cfg(feature = "savestates")]
    pub fn loadstate<P: AsRef<Path>>(&mut self, path: P) -> Result<(), LoadError> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let mut new_emu: NesEmulator = pot::from_reader(reader)?;
        // let mut new_emu: NesEmulator = pot::from_reader(file)?;

        use std::mem;

        mem::swap(&mut self.mem.prg, &mut new_emu.mem.prg);
        if !new_emu.rom_info().has_chr_ram {
            mem::swap(&mut self.mem.chr, &mut new_emu.mem.chr);
        }

        mem::swap(&mut self.output, &mut new_emu.output);
        *self = new_emu;

        Ok(())
    }
}
