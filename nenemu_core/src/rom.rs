use crate::{
    emu::{Mirroring, Region},
    games_db::GAMES_DB,
};

pub struct Cart {
    pub header: RomData,
    pub prg: Vec<u8>,
    pub chr: Vec<u8>,
}
impl Default for Cart {
    // empty cart with zeroed prg and chr
    fn default() -> Self {
        Self {
            header: RomData {
                prg_size: 16 * 1024,
                chr_size: 8 * 1024,
                ..Default::default()
            },
            prg: vec![0; 16 * 1024],
            chr: vec![0; 8 * 1024],
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
pub enum HeaderFormat {
    #[default]
    Headerless,
    INes,
    Nes2_0,
    Fds,
}

// https://www.nesdev.org/wiki/INES
#[derive(Debug, Clone)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
pub struct RomData {
    pub title: String,
    pub format: HeaderFormat,
    pub mapper: u16,
    pub submapper: u8,
    pub region: Region,

    pub prg_size: usize,
    pub chr_size: usize,
    pub wram_size: usize,

    pub mirroring: Mirroring,
    pub alt_mirroring: bool,

    pub expansions: u8,

    pub has_battery: bool,
    pub has_chr_ram: bool,
    pub has_trainer: bool,
}

impl Default for RomData {
    fn default() -> Self {
        Self {
            title: String::new(),
            format: HeaderFormat::Headerless,
            mapper: 0,
            submapper: 0,
            region: Region::NTSC,

            prg_size: 0,
            chr_size: 0,
            wram_size: 0,

            mirroring: Default::default(),
            alt_mirroring: false,

            expansions: 0,
            has_battery: false,
            has_chr_ram: false,
            has_trainer: false,
        }
    }
}

impl RomData {
    const INES_MAGIC: &[u8] = &[0x4e, 0x45, 0x53, 0x1a];
    const INES_HEADER_SIZE: usize = 16;
    const UNIF_MAGIC: &[u8] = &[0x55, 0x4e, 0x49, 0x46];
    const UNIF_HEADER_SIZE: usize = 32;
    const TRAINER_SIZE: usize = 512;

    pub fn is_valid_ines(bytes: &[u8]) -> bool {
        bytes.len() >= Self::INES_HEADER_SIZE && &bytes[0..4] == Self::INES_MAGIC
    }

    pub fn is_valid_unif(bytes: &[u8]) -> bool {
        bytes.len() >= Self::UNIF_HEADER_SIZE && &bytes[0..4] == Self::UNIF_MAGIC
    }

    pub fn len(&self) -> usize {
        match self.format {
            HeaderFormat::Headerless | HeaderFormat::Fds => 0,
            HeaderFormat::INes | HeaderFormat::Nes2_0 => {
                if self.has_trainer {
                    Self::INES_HEADER_SIZE + Self::TRAINER_SIZE
                } else {
                    Self::INES_HEADER_SIZE
                }
            }
        }
    }

    pub fn from_db(bytes: &[u8]) -> Result<Self, &'static str> {
        let header = Self::parse(bytes);

        match header {
            Ok(mut header) => {
                // query additional information about rom
                let entry = GAMES_DB.query(&bytes[header.len()..]);
                if let Some(entry) = entry {
                    // we do not trust INes, get all data from database
                    if header.format == HeaderFormat::INes {
                        header = entry.into();
                        header.format = HeaderFormat::INes;
                    }

                    header.title = entry.title.clone();
                }

                Ok(header)
            }

            // headerless rom: query database
            Err(e) => GAMES_DB.query(bytes).ok_or(e).map(|x| x.into()),
        }
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
        if Self::is_valid_unif(bytes) {
            return Err("valid UNIF ROM, but not supported by this emulator");
        }

        if !Self::is_valid_ines(bytes) {
            return Err("not a valid iNES/NES2.0 ROM");
        }

        let mut header = RomData::default();
        header.format = HeaderFormat::INes;

        header.prg_size = bytes[4] as usize * 16 * 1024;
        header.has_chr_ram = bytes[5] == 0;
        header.chr_size = bytes[5] as usize * 8 * 1024;
        // default wram to 8kb if iNes, as we can't tell
        header.wram_size = 8 * 1024;

        header.mapper = ((bytes[7] & 0xf0) | (bytes[6] >> 4)) as u16;
        header.has_battery = bytes[6] & 0x2 != 0;
        header.has_trainer = bytes[6] & 0x4 != 0;
        header.alt_mirroring = bytes[6] & 0x8 != 0;

        header.mirroring = match bytes[6] & 1 {
            0 => Mirroring::Horizontal,
            _ => Mirroring::Vertical,
        };

        let version = bytes[7] & 0xc;

        if version == 0x08 {
            // NES 2.0
            if bytes[9] & 0xf == 0x0f || bytes[9] & 0xf0 == 0xf0 {
                return Err("exponent-multiplier notation not supported");
            }

            header.format = HeaderFormat::Nes2_0;
            header.mapper |= (bytes[8] as u16 & 0xf) << 8;
            header.submapper = bytes[8] >> 4;

            let prg_ram_shift = bytes[10] & 0xf;
            let prg_nvram_shift = bytes[10] >> 4;

            // MMC5 games might have two different ram chips, we take the sum of sram and wram
            // https://www.nesdev.org/wiki/MMC5#PRG-RAM_configurations
            let prg_ram_size = if prg_ram_shift > 0 {
                64 << prg_ram_shift
            } else {
                0
            };
            let prg_nvram_size = if prg_nvram_shift > 0 {
                64 << prg_nvram_shift
            } else {
                0
            };
            // we only take nvram is ram is zero
            header.wram_size = prg_ram_size + prg_nvram_size;

            // we only take chr ram if chr rom is zero
            if header.chr_size == 0 {
                let chr_ram_shift = bytes[11] & 0xf;
                let chr_nvram_shift = bytes[11] >> 4;

                header.chr_size = if chr_ram_shift > 0 {
                    64 << chr_ram_shift
                } else if chr_nvram_shift > 0 {
                    64 << chr_nvram_shift
                } else {
                    8 * 1024
                };
            }

            header.region = match bytes[12] & 0b11 {
                1 | 3 => Region::PAL,
                _ => Region::NTSC,
            };

            header.expansions = bytes[15] & 0x3f;
        } else if version == 0 && bytes[12..=15].iter().all(|x| *x == 0) {
            // https://www.nesdev.org/wiki/INES#Variant_comparison
            // iNES with PRG RAM and TV system field
            header.wram_size = bytes[8] as usize * 8 * 1024;
            if header.has_battery && header.wram_size == 0 {
                // default it to 32kb, nothing we can do
                header.wram_size = 32 * 1024;
            }

            header.region = match bytes[9] {
                1 => Region::PAL,
                _ => Region::NTSC,
            };
        }

        // if chr rom is 0, default to 8kb
        header.chr_size = if header.chr_size == 0 {
            8 * 1024
        } else {
            header.chr_size
        };

        Ok(header)
    }
}

impl Cart {
    pub fn from(bytes: &[u8]) -> Result<Self, &'static str> {
        let header = RomData::from_db(bytes)?;

        // only iNes supported
        let rom_start = header.len();
        let prg = bytes[rom_start..rom_start + header.prg_size].to_vec();
        let chr = if header.has_chr_ram {
            vec![0; header.chr_size]
        } else {
            let chr_start = rom_start + header.prg_size;
            bytes[chr_start..chr_start + header.chr_size].to_vec()
        };

        Ok(Self { header, prg, chr })
    }
}

#[derive(Default)]
pub struct Disk {
    pub sides_bytes: Vec<Box<[u8]>>,
    pub sides_data: Vec<SideData>,
}

#[derive(Default, Debug)]
pub enum Side {
    #[default]
    SideA,
    SideB,
}

#[derive(Default, Debug)]
pub struct SideData {
    title: String,
    disk_side: Side,
    disk_number: u8,
    files_count: u8,
    files: Vec<FileData>,
}

#[derive(Default, Debug)]
pub enum FileKind {
    #[default]
    PRAM,
    CRAM,
    VRAM,
}

#[derive(Default, Debug)]
pub struct FileData {
    number: u8,
    id: u8,
    name: String,
    address: u16,
    size: u16,
    kind: FileKind,
}

// https://github.com/SourMesen/Mesen2/blob/fabc9a62174f8734a113df6d244f5539ef6b8fcf/Core/NES/Loaders/FdsLoader.cpp#L21
// https://github.com/ares-emulator/ares/blob/0b2a85f80321aca7af9df37555edfdd5c4d22a9c/mia/medium/famicom-disk-system.cpp
// https://forums.nesdev.org/viewtopic.php?t=18668
// https://forums.nesdev.org/viewtopic.php?f=3&t=8712
impl Disk {
    const FDS_MAGIC: &[u8] = &[0x46, 0x44, 0x53, 0x1A];
    const FDS_NINTENDO_STR: &[u8] = "*NINTENDO-HVC*".as_bytes();
    const FDS_HEADER_SIZE: usize = 16;
    const SIDE_SIZE: usize = 65500;

    fn push_gaps_and_data(data: &mut Vec<u8>, block: &[u8]) {
        // Gap between blocks : At least 480 bits, 976 bits typical.
        data.extend(std::iter::repeat_n(0, 976 / 8));
        // Gaps are teminated by a single '1' bit. In terms of bytes, it would be $80, as the data is stored in little endian format.
        data.push(0x80);

        data.extend_from_slice(block);
        // At the end of each block, a 16-bit CRC is stored.
        // fake CRC value
        data.push(0xde);
        data.push(0xad);
    }

    pub fn is_valid_fds(bytes: &[u8]) -> bool {
        let (rom_start, sides_count) = if &bytes[..4] == Self::FDS_MAGIC {
            (Self::FDS_HEADER_SIZE, bytes[4] as usize)
        } else {
            (0, bytes.len() / Self::SIDE_SIZE)
        };

        if sides_count == 0 {
            return false;
        }

        // we only check for the first side nintendo bytes
        bytes[rom_start] == 1 && &bytes[rom_start + 1..rom_start + 15] == Self::FDS_NINTENDO_STR
    }

    pub fn from(bytes: &[u8]) -> Result<Self, &'static str> {
        let (rom_start, sides_count) = if &bytes[..4] == Self::FDS_MAGIC {
            (Self::FDS_HEADER_SIZE, bytes[4] as usize)
        } else {
            (0, bytes.len() / Self::SIDE_SIZE)
        };

        if sides_count == 0 {
            return Err("not a valid FDS rom");
        }

        let mut sides_bytes = Vec::new();
        let mut sides_data = Vec::new();

        let mut img = &bytes[rom_start..];
        for _ in 0..sides_count {
            let mut side_bytes = Vec::with_capacity(Self::SIDE_SIZE);

            // Physically on the disk, there are "gaps" of 0 recorded between blocks and before the start of the disk. Length of the gaps are as follows:
            // Before the start of the disk : At least 26150 bits, 28300 typical.
            side_bytes.resize(28300 / 8, 0);
            side_bytes.push(0x80);

            if img[0] != 1 {
                return Err("no valid side info block");
            }
            if &img[1..15] != Self::FDS_NINTENDO_STR {
                return Err("not a valid FDS rom");
            }

            let mut side_data = SideData::default();
            side_data.title = String::from_utf8_lossy(&img[0x10..0x13]).into_owned();
            side_data.disk_side = if img[0x15] == 0 {
                Side::SideA
            } else {
                Side::SideB
            };
            side_data.disk_number = img[0x16];

            // disk info block is 0x38 (56) bytes
            side_bytes.extend_from_slice(&img[..0x38]);
            side_bytes.push(0xde);
            side_bytes.push(0xad);

            if img[0x38] != 2 {
                return Err("no valid file amount block");
            }

            let files_count = img[0x39];
            side_data.files_count = files_count;

            // file info block is 2 bytes
            Self::push_gaps_and_data(&mut side_bytes, &img[0x38..0x3a]);

            let mut file = &img[0x3a..];
            let mut parsed_bytes = 0x3a;
            for _ in 0..files_count {
                // if no more files are found, simply stop and fill rest of disk with zeroes
                if file[0] != 3 {
                    break;
                }

                let mut file_data = FileData::default();

                file_data.number = file[1];
                file_data.id = file[2];
                file_data.name = String::from_utf8_lossy(&file[0x3..0xb])
                    .into_owned()
                    .trim_end_matches(|c: char| c.is_control())
                    .to_string();

                file_data.address = u16::from_le_bytes([file[0xb], file[0xc]]);
                file_data.size = u16::from_le_bytes([file[0xd], file[0xe]]);
                file_data.kind = match file[0xf] {
                    0 => FileKind::PRAM,
                    1 => FileKind::CRAM,
                    _ => FileKind::VRAM,
                };

                let file_size = file_data.size as usize;

                // file header block is 0x10 (16) bytes
                Self::push_gaps_and_data(&mut side_bytes, &file[..0x10]);

                if file[0x10] != 4 {
                    break;
                }

                Self::push_gaps_and_data(&mut side_bytes, &file[0x10..0x10 + file_size + 1]);

                // TODO: handle case when we go over 65500 bytes
                parsed_bytes += 0x10 + file_size + 1;
                if parsed_bytes > Self::SIDE_SIZE {
                    return Err("Side data is bigger than 65500 bytes");
                }

                file = &file[0x10 + file_size + 1..];

                side_data.files.push(file_data);
            }

            // After the last file block, fill a side with all 0 so that the disk side reaches exactly 65500 bytes.
            img = &img[Self::SIDE_SIZE..];
            if side_bytes.len() < Self::SIDE_SIZE {
                side_bytes.resize(Self::SIDE_SIZE, 0);
            }

            sides_bytes.push(side_bytes.into_boxed_slice());
            sides_data.push(side_data);
        }

        Ok(Self {
            sides_bytes,
            sides_data,
        })
    }
}
