use std::{
    io::{self, Read},
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
    rom::{Cart, Disk, RomData, is_valid_bios, is_valid_fds, is_valid_ines},
    utils::{AvgResampler, RingBuffer},
};

pub const NTSC_CLOCK_RATE: usize = 1789773;
pub const PAL_CLOCK_RATE: usize = 1662607;

pub const NTSC_FRAME_RATE: f32 = 60.0988;
pub const PAL_FRAME_RATE: f32 = 50.0070;

pub const SCREEN_WIDTH: isize = 256;
pub const SCREEN_HEIGHT: isize = 240;

pub const FRAMEBUF_SIZE: usize = SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize * 4;
pub const AUDIO_FRAMES_BUFFERED: usize = 8;

pub const BATTERY_SAVE_EXTENSION: &str = "srm";
pub(crate) type LoadError = Box<dyn std::error::Error>;

#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
pub struct NesEmulator {
    pub cpu: Cpu6502,
    pub ppu: Ppu2C02,
    pub apu: ApuRP2A,
    pub joy: Joypad,
    pub mem: Bus,
    pub mapper: Box<dyn Mapper>,

    #[cfg_attr(feature = "savestates", serde(skip))]
    pub output: NesOutput,

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

#[derive(Debug, Default, Clone, Copy, PartialEq, bitcode::Encode, bitcode::Decode)]
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

impl NesEmulator {
    pub fn empty() -> Self {
        Self::new(Game::default(), None, NesSettings::default()).unwrap()
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

    fn new(game: Game, bios: Option<Vec<u8>>, settings: NesSettings) -> Result<Self, LoadError> {
        let (mem, mapper) = match game {
            Game::Cart(cart) => {
                let mut mem = Bus::with_cart(cart);
                let mapper = mapper::new(&mut mem)?;
                (mem, mapper)
            }
            Game::Disk(disk) => {
                let bios = bios.ok_or("no FDS BIOS provided")?;

                if !is_valid_bios(&bios) {
                    return Err("not a valid FDS BIOS provided".into());
                }
                Bus::with_disk(disk, bios)
            }
        };

        let mut emu = Self {
            output: NesOutput::new(&mem.header.region),

            cpu: Cpu6502::new(),
            ppu: Ppu2C02::new(&mem.header.region),
            apu: ApuRP2A::new(&mem.header.region),
            joy: Joypad::default(),
            mem,
            mapper,

            palette: NesPalette::default(),
            settings,
        };

        if emu.settings.random_ram {
            // Final Fantasy, River City Ransom, Apple Town Story[5], Impossible Mission II[6] amongst others
            // Use the semi-random contents of RAM on powerup to seed their RNGs.
            getrandom::fill(&mut emu.mem.ram)?;
            getrandom::fill(&mut emu.mem.wram)?;
        }

        // Start from RST interrupt handler
        emu.cpu.pc = emu.cpu_read16(cpu::InterruptVector::Rst as u16);
        Ok(emu)
    }

    pub fn bios_only<B: AsRef<[u8]>>(bios: B) -> Result<Self, LoadError> {
        Self::builder()
            .with_fds_bios(Some(&bios.as_ref()))
            .boot_bios_only(true)
            .build()
    }

    pub fn builder<'a>() -> NesBuilder<'a> {
        NesBuilder::default()
    }

    pub fn region(&self) -> Region {
        self.mem.header.region
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

    pub fn try_set_palette<B: AsRef<[u8]>>(&mut self, bytes: B) -> Result<(), &str> {
        if let Some(pal) = NesPalette::from_pal_file_bytes(bytes.as_ref()) {
            self.set_palette(pal);
            Ok(())
        } else {
            Err("not a valid NES palette file")
        }
    }

    pub fn set_palette(&mut self, pal: NesPalette) {
        self.palette = pal;
    }

    pub fn set_settings(&mut self, settings: NesSettings) {
        self.settings = settings;
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
        if self.region() == Region::PAL {
            if self.cpu.cycles % 5 == 0 {
                self.ppu_step();
            }
        }
    }

    pub fn check_for_errrors(&self) -> Result<(), &'static str> {
        self.cpu
            .jammed
            .not()
            .then(|| ())
            .ok_or("cpu panicked (reached a jam instruction or unimplemented illegal)")
    }

    pub fn step(&mut self) {
        self.cpu_step();
    }

    pub fn step_until_frame_ready(&mut self) -> Result<(), &'static str> {
        while self.is_frame_ready() {
            self.cpu_step();
        }

        while !self.is_frame_ready() {
            self.cpu_step();
        }

        self.check_for_errrors()
    }

    pub fn step_until_samples_ready(&mut self, samples_amt: usize) -> Result<(), &'static str> {
        while self.audio_queued() < samples_amt {
            self.cpu_step();
        }

        self.check_for_errrors()
    }

    pub fn emu_reset(&mut self) {
        self.cpu_reset();
        self.ppu.reset();
        self.apu.reset();

        // TODO: should reload wram battery!
        // TODO: some mappers need to be reset too
    }

    pub fn is_frame_ready(&self) -> bool {
        self.output.frame_ready
    }

    pub fn frame_number(&self) -> usize {
        self.output.frame_number
    }

    pub fn get_video_rgba(&self) -> &[u8; FRAMEBUF_SIZE] {
        // if self.output.frame_ready {
        //     self.output.frame_ready = false;
        // }
        &self.output.videobuf_view.0
    }

    pub fn put_video_rgba(&self, buf: &mut [u8]) {
        // if self.output.frame_ready {
        //     self.output.frame_ready = false;
        // }

        buf.copy_from_slice(self.get_video_rgba());
    }

    // 256 * 240 * 4 * 4 texture needed
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

    pub fn audio_queued(&self) -> usize {
        // queued : CLOCKRATE = x : TargetRate
        // (self.output.audiobuf.queued() as f64 * rate as f64 / self.clock_rate() as f64).round()
        //     as usize
        self.output.audiobuf.queued()
    }

    pub fn set_audio_rate(&mut self, rate: f32) {
        self.output
            .resampler
            .set_rate(self.region().clock_rate(), rate);
    }

    pub fn get_audio_f32(&mut self, amount: usize) -> (&[f32], Option<&[f32]>) {
        self.output.audiobuf.take(amount)
    }

    pub fn get_audio_f32_iter(&mut self, amount: usize) -> impl Iterator<Item = &f32> {
        self.output.audiobuf.take_iter(amount)
    }

    pub fn get_audiobuf(&mut self) -> &mut RingBuffer<f32> {
        &mut self.output.audiobuf
    }

    pub fn put_audio_f32(&mut self, buf: &mut [f32]) {
        let (right, left) = self.output.audiobuf.take(buf.len());
        buf[..right.len()].copy_from_slice(right);

        if let Some(left) = left {
            buf[right.len()..].copy_from_slice(left);
        }
    }

    pub fn save_battery(&self) -> Option<&[u8]> {
        // TODO: FDS disk save

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
        // TODO: FDS disk load

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

#[derive(Default)]
pub struct NesOutput {
    pub(crate) frame_ready: bool,
    pub(crate) frame_number: usize,
    // These get reallocated every time an emulator is created, consider changing to Vector
    pub(crate) videobuf_back: Box<Framebuf>,
    pub(crate) videobuf_view: Box<Framebuf>,
    pub audiobuf: RingBuffer<f32>,
    pub resampler: AvgResampler,
}
impl NesOutput {
    pub fn new(region: &Region) -> Self {
        Self {
            audiobuf: RingBuffer::new(
                (AUDIO_FRAMES_BUFFERED as f32 * (region.clock_rate() as f32 / region.frame_rate()))
                    as usize,
            ),
            resampler: AvgResampler::new(region.clock_rate(), SampleRate::default()),
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

#[derive(Clone, PartialEq)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
pub struct NesSettings {
    pub random_ram: bool,
    pub disable_sprite_limit: bool,
    pub enable_accurate_ppu: bool,

    pub enable_background: bool,
    pub enable_sprites: bool,

    // TODO: not implemented
    pub pal_borders: bool,

    pub enable_pulse0: bool,
    pub enable_pulse1: bool,
    pub enable_triangle: bool,
    pub enable_noise: bool,
    pub enable_dmc: bool,
    pub enable_ext_audio: bool,
}
impl Default for NesSettings {
    fn default() -> Self {
        Self {
            random_ram: true,
            disable_sprite_limit: true,
            enable_accurate_ppu: false,

            enable_background: true,
            enable_sprites: true,

            pal_borders: true,

            enable_pulse0: true,
            enable_pulse1: true,
            enable_triangle: true,
            enable_noise: true,
            enable_dmc: true,
            enable_ext_audio: true,
        }
    }
}

enum Game {
    Cart(Cart),
    Disk(Disk),
}

impl Default for Game {
    // default to empty cartrdige (all NOPs)
    fn default() -> Self {
        Self::Cart(Cart::default())
    }
}

impl Game {
    pub fn from_bytes<B: AsRef<[u8]>>(bytes: B) -> Result<Self, LoadError> {
        let bytes = bytes.as_ref();

        if is_valid_ines(bytes) {
            Ok(Game::Cart(Cart::from_bytes(bytes)?))
        } else if is_valid_fds(bytes) {
            Ok(Game::Disk(Disk::from_bytes(bytes)?))
        } else {
            // might be headless rom
            Ok(Game::Cart(Cart::from_bytes(bytes)?))
        }
    }
}

pub fn read_file_maybe_zipped<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);
    let mut bytes = Vec::new();

    match zip::read::ZipArchive::new(&mut reader) {
        Ok(mut archive) => {
            // it is a zip file
            let mut zip = archive.by_index(0)?;
            zip.read_to_end(&mut bytes)?;
            io::Result::Ok(bytes)
        }

        Err(_) => {
            // not a zip file
            use std::io::Seek;

            reader.rewind()?;
            bytes.clear();
            reader.read_to_end(&mut bytes)?;
            io::Result::Ok(bytes)
        }
    }
}

pub fn read_zip_file_from_bytes<B: AsRef<[u8]>>(input: B) -> io::Result<Vec<u8>> {
    let mut reader = io::BufReader::new(input.as_ref());
    let unzipped = zip::read::read_zipfile_from_stream(&mut reader)?;

    match unzipped {
        Some(mut zipfile) => {
            let mut buf = Vec::new();
            zipfile.read_to_end(&mut buf)?;
            io::Result::Ok(buf)
        }
        None => {
            let err = io::Error::new(
                io::ErrorKind::IsADirectory,
                "file was not present at root of zip directory",
            );
            io::Result::Err(err)
        }
    }
}

pub fn read_bytes_maybe_zipped<B: AsRef<[u8]>>(input: B) -> Vec<u8> {
    read_zip_file_from_bytes(&input).unwrap_or(input.as_ref().to_owned())
}

pub fn read_file_buffered<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let mut reader = io::BufReader::new(file);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    Ok(bytes)
}

enum RomSource<'a> {
    Bytes(&'a [u8]),
    FilePath(&'a Path),
}

#[derive(Default)]
pub struct NesBuilder<'a> {
    rom: Option<RomSource<'a>>,
    bios: Option<RomSource<'a>>,
    boot_bios_only: bool,

    palette: NesPalette,
    settings: NesSettings,
}

impl<'a> NesBuilder<'a> {
    pub fn with_rom<R: 'a + AsRef<[u8]>>(mut self, rom: &'a R) -> Self {
        self.rom = Some(RomSource::Bytes(rom.as_ref()));
        self
    }

    pub fn with_rom_file<P: 'a + AsRef<Path>>(mut self, rom_path: &'a P) -> Self {
        self.rom = Some(RomSource::FilePath(rom_path.as_ref()));
        self
    }

    pub fn with_fds_bios<B: 'a + AsRef<[u8]>>(mut self, bios: Option<&'a B>) -> Self {
        self.bios = bios.map(|bios| RomSource::Bytes(bios.as_ref()));
        self
    }

    pub fn with_fds_bios_file<P: 'a + AsRef<Path>>(mut self, bios_path: Option<&'a P>) -> Self {
        self.bios = bios_path.map(|path| RomSource::FilePath(path.as_ref()));
        self
    }

    pub fn boot_bios_only(mut self, cond: bool) -> Self {
        self.boot_bios_only = cond;
        self
    }

    pub fn with_settings(mut self, settings: NesSettings) -> Self {
        self.settings = settings;
        self
    }

    pub fn with_palette(mut self, palette: NesPalette) -> Self {
        self.palette = palette;
        self
    }

    pub fn build_empty(self) -> NesEmulator {
        NesEmulator::empty()
    }

    pub fn build_with_rom<R: 'a + AsRef<[u8]>>(self, rom: R) -> Result<NesEmulator, LoadError> {
        Self::default().with_rom(&rom).build()
    }

    pub fn build(self) -> Result<NesEmulator, LoadError> {
        // games might be zipped!

        if self.boot_bios_only {
            return match self.bios {
                Some(bios) => {
                    let bios = match bios {
                        RomSource::Bytes(bytes) => read_zip_file_from_bytes(bytes)
                            .map_or_else(|_| bytes.to_owned(), |unzipped| unzipped),
                        RomSource::FilePath(path) => read_file_maybe_zipped(path)?,
                    };

                    let empty_disk = Game::Disk(Disk::default());
                    NesEmulator::new(empty_disk, Some(bios), self.settings)
                }

                None => Err("no FDS BIOS provided".into()),
            };
        }

        let game = if let Some(rom) = self.rom {
            match rom {
                RomSource::Bytes(bytes) => read_zip_file_from_bytes(bytes).map_or_else(
                    |_| Game::from_bytes(bytes),
                    |unzipped| Game::from_bytes(&unzipped),
                )?,
                RomSource::FilePath(path) => read_file_maybe_zipped(path)
                    .map_err(|e| e.into())
                    .and_then(|res| Game::from_bytes(res))?,
            }
        } else {
            eprintln!("Error reading rom file. Defaulting game struct");
            Game::default()
        };

        match &game {
            // ignore bios
            Game::Cart(_) => NesEmulator::new(game, None, self.settings),

            // bios needed
            Game::Disk(_) => match self.bios {
                Some(bios) => {
                    let bios = match bios {
                        RomSource::Bytes(bytes) => read_zip_file_from_bytes(bytes)
                            .map_or_else(|_| bytes.to_owned(), |unzipped| unzipped),
                        RomSource::FilePath(path) => read_file_maybe_zipped(path)?,
                    };

                    NesEmulator::new(game, Some(bios), self.settings)
                }

                None => Err("detected FDS game; BIOS required but not provided".into()),
            },
        }
    }
}
