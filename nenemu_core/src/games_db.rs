use crate::{
    emu::{Mirroring, Region},
    rom::{HeaderFormat, RomData, get_mapper_name},
};
use std::{collections::HashMap, io::Read, sync::LazyLock};

// TODO: no support for VsSystem yet
// enum Console {
//     NES,
//     PlayChoice10,
//     VsSystem { ppu: u8, hw: u8 },
//     Extended(u8),
// }

#[derive(Debug, bitcode::Encode, bitcode::Decode)]
pub struct GameDbEntry {
    pub title: String,

    pub rom_total_size: usize,
    pub rom_crc32: u32,
    // pub rom_sha1: String,
    // pub rom_sum16: String,
    // pub rom_sha1: [u8; 20],
    pub prg_size: usize,
    pub prg_crc32: u32,
    // pub prg_sha1: String,
    // pub prg_sum16: String,
    // pub prg_sha1: [u8; 20],
    pub chr_size: usize,
    pub chrram_size: usize,
    pub prgram_size: usize,
    pub prgnvram_size: usize,
    // no game has chrnvram
    // pub chrnvram_size: usize,
    pub mapper: u16,
    pub submapper: u8,
    pub mirroring: Mirroring,
    pub region: Region,
    pub has_battery: bool,

    pub console: u8,
    pub expansions: u8,
}
impl From<&GameDbEntry> for RomData {
    fn from(value: &GameDbEntry) -> Self {
        let chr_size = if value.chr_size > 0 {
            value.chr_size
        } else if value.chrram_size > 0 {
            value.chrram_size
        } else {
            8 * 1024
        };

        let wram_size = value.prgram_size + value.prgnvram_size;

        Self {
            title: value.title.clone(),
            prg_size: value.prg_size,
            chr_size,
            wram_size,
            has_chr_ram: value.chrram_size > 0,
            mirroring: value.mirroring.clone(),
            region: value.region.clone(),
            mapper: value.mapper,
            mapper_name: get_mapper_name(value.mapper).into(),
            submapper: value.submapper,
            has_battery: value.has_battery,
            expansions: value.expansions,

            // if we made a header here, it means it was headerless
            format: HeaderFormat::Headerless,

            alt_mirroring: false,
            has_trainer: false,
        }
    }
}

pub struct GamesDb {
    games: Vec<GameDbEntry>,
    rom_map: HashMap<u32, usize>,
    prg_map: HashMap<u32, usize>,
}
impl GamesDb {
    pub fn new(games: Vec<GameDbEntry>) -> Self {
        let rom_map = games
            .iter()
            .enumerate()
            // .map(|(i, e)| (e.rom_sha1.clone(), i))
            .map(|(i, e)| (e.rom_crc32, i))
            .collect::<HashMap<_, _>>();

        let prg_map = games
            .iter()
            .enumerate()
            // .map(|(i, e)| (e.prg_sha1.clone(), i))
            .map(|(i, e)| (e.prg_crc32, i))
            .collect::<HashMap<_, _>>();

        Self {
            games,
            rom_map,
            prg_map,
        }
    }

    pub fn query(&self, rom: &[u8]) -> Option<&GameDbEntry> {
        // let mut sha1 = sha1_smol::Sha1::new();
        // sha1.update(&rom[16..]);
        // let rom_hash = sha1.digest().bytes();

        let rom_hash = crc32fast::hash(&rom);

        let index = self.rom_map.get(&rom_hash).or_else(move || {
            // rom hash not found, try parsing the header and hash prg
            let header = RomData::parse(rom);
            let prg_size = header.map_or(None, |x| Some(x.prg_size));

            match prg_size {
                Some(prg_size) => {
                    // sha1.reset();
                    // sha1.update(&rom[16..16+prg_size]);
                    // let prg_hash = sha1.digest().bytes();

                    let prg_hash = crc32fast::hash(&rom[..prg_size]);
                    self.prg_map.get(&prg_hash)
                }

                None => None,
            }
        });

        index.and_then(|i| Some(&self.games[*i]))
    }
}

// https://forums.nesdev.org/viewtopic.php?t=19940
pub static GAMES_DB: LazyLock<GamesDb> = LazyLock::new(|| {
    let db = include_bytes!("../utils/nes20db.bitcode.gzip").as_slice();

    let mut encoded = flate2::read::GzDecoder::new(db);
    let mut buf = Vec::new();
    encoded.read_to_end(&mut buf).unwrap();
    let games: Vec<GameDbEntry> = bitcode::decode(&buf).unwrap();

    GamesDb::new(games)
});

#[cfg(test)]
mod tests {
    use crate::games_db::{GAMES_DB, GameDbEntry};
    use std::{collections::BTreeSet, io::Read};

    #[test]
    fn db_access_test() {
        println!("Number of entries: {}", GAMES_DB.games.len());

        let rom = include_bytes!("../../roms/metroid.nes");
        let res = GAMES_DB.query(&rom[16..]);

        println!("{res:?}");
    }

    #[test]
    fn decode_test() {
        let file = include_bytes!("../utils/nes20db.bitcode.gzip").as_slice();

        let mut decode = flate2::read::GzDecoder::new(file);
        let mut buf = Vec::new();
        decode.read_to_end(&mut buf).unwrap();

        let _: Vec<GameDbEntry> = bitcode::decode(&buf).unwrap();
    }

    #[test]
    fn count_prgram() {
        let both = GAMES_DB
            .games
            .iter()
            .filter(|x| x.prgram_size > 0 && x.prgnvram_size > 0)
            .count();
        let only_prgram = GAMES_DB.games.iter().filter(|x| x.prgram_size > 0).count();
        let only_prgnvram = GAMES_DB
            .games
            .iter()
            .filter(|x| x.prgnvram_size > 0)
            .count();

        dbg!(both);
        dbg!(only_prgram);
        dbg!(only_prgnvram);

        let interesting = GAMES_DB
            .games
            .iter()
            .filter_map(|x| {
                if x.prgram_size > 0 && x.prgnvram_size > 0 && x.mapper <= 255 {
                    Some((&x.title, x.mapper, x.prgram_size, x.prgnvram_size))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        println!("{:#?}", interesting);

        let games_with_both = GAMES_DB
            .games
            .iter()
            .filter(|x| x.prgram_size > 0 && x.prgnvram_size > 0)
            .map(|x| x.mapper)
            .collect::<BTreeSet<_>>();
        println!("{:#?}", games_with_both);
    }

    #[test]
    fn count_chr() {
        let both = GAMES_DB
            .games
            .iter()
            .filter(|x| x.chr_size > 0 && x.chrram_size > 0)
            .count();
        let only_chr = GAMES_DB.games.iter().filter(|x| x.chr_size > 0).count();
        let only_chrram = GAMES_DB.games.iter().filter(|x| x.chrram_size > 0).count();

        dbg!(both);
        dbg!(only_chr);
        dbg!(only_chrram);

        let interesting = GAMES_DB
            .games
            .iter()
            .filter(|x| x.chr_size > 0 && x.chrram_size > 0 && x.mapper <= 5 || x.mapper == 119)
            .collect::<Vec<_>>();

        println!("{:#?}", interesting);

        let games_with_both = GAMES_DB
            .games
            .iter()
            .filter(|x| x.chr_size > 0 && x.chrram_size > 0)
            .map(|x| x.mapper)
            .collect::<BTreeSet<_>>();
        println!("{:#?}", games_with_both);
    }

    #[test]
    fn count_mmc5_ram() {
        let count = GAMES_DB
            .games
            .iter()
            .filter(|x| x.mapper == 5 && x.chrram_size > 0)
            .collect::<Vec<_>>();
        println!("{count:#?}");
    }

    #[test]
    fn bios_crc32() {
        println!(
            "{}",
            crc32fast::hash(include_bytes!("../utils/disksys.rom"))
        )
    }
}
