use std::{collections::{HashMap, HashSet}, fs, io::{Read, Write}};

use nes_emulator::games_db::GameData;

#[derive(serde::Serialize, serde::Deserialize)]
struct XmlRoot {
  nes20db: XmlGameList, 
}

#[derive(serde::Serialize, serde::Deserialize)]
struct XmlGameList {
  game: Vec<XmlGameEntry>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct XmlGameEntry {
  prgrom: XmlRomData,
  chrrom: Option<XmlRomData>,
  prgram: Option<XmlRamData>,
  prgnvram: Option<XmlRamData>,
  chrram: Option<XmlRamData>,
  chrnvram: Option<XmlRamData>,
  miscrom: Option<XmlRomDataMisc>,

  rom: XmlRomData,
  pcb: XmlCartData,
  console: XmlConsoleData,
  expansion: XmlExpansionData,
  vs: Option<XmlVsData>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct XmlVsData {
  _hardware: String,
  _ppu: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct XmlRamData {
  _size: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct XmlCartData {
  _mapper: String,
  _submapper: String,
  _mirroring: String,
  _battery: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct XmlExpansionData {
  #[serde(rename = "_type")]
  kind: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct XmlConsoleData {
  #[serde(rename = "_type")]
  kind: String,
  #[serde(rename = "_region")]
  region: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct XmlRomData {
  #[serde(rename = "_size")]
  size: String,
  #[serde(rename = "_crc32")]
  crc32: String,
  #[serde(rename = "_sha1")]
  sha1: String,
  #[serde(rename = "_sum16", default)]
  sum16: String,
}

#[derive(Default, Debug, serde::Serialize, serde::Deserialize, bincode::Encode, bitcode::Encode, bitcode::Decode)]
struct XmlRomDataMisc {
  #[serde(rename = "_size")]
  size: String,
  #[serde(rename = "_crc32")]
  crc32: String,
  #[serde(rename = "_sha1")]
  sha1: String,
  #[serde(rename = "_sum16", default)]
  sum16: String,
  #[serde(rename = "_number")]
  number: String,
}


fn sha1_str_to_arr(s: &str) -> [u8; 20] {
  let mut vec = Vec::new();
  for pair in s.as_bytes().chunks(2) {
    let str = String::from_iter(pair.iter().map(|b| *b as char));
    let num =  u8::from_str_radix(&str, 16).unwrap();
    vec.push(num);
  }

  vec.try_into().unwrap()
}

fn crc32_str_to_int(s: &str) -> u32 {
  u32::from_str_radix(s, 16).unwrap()
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize, bitcode::Encode, bitcode::Decode, bincode::Encode)]
struct VsSystem {
  hardware: u16,
  ppu: u16,
}
impl From<&XmlVsData> for VsSystem {
  fn from(value: &XmlVsData) -> Self {
    Self {
      hardware: value._hardware.parse().unwrap(),
      ppu: value._ppu.parse().unwrap(),
    }
  }
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize, bitcode::Encode, bitcode::Decode, bincode::Encode)]
struct RomSection {
  size: usize,
  crc32: String,
  sha1: String,
  sum16: String
}
impl From<&XmlRomData> for RomSection {
  fn from(value: &XmlRomData) -> Self {
    Self {
      size: value.size.parse().unwrap(),
      crc32: value.crc32.clone(),
      sha1: value.sha1.clone(),
      sum16: value.sum16.clone(),
    }
  }
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize, bitcode::Encode, bitcode::Decode, bincode::Encode)]
struct RomSectionMisc {
  size: usize,
  crc32: String,
  sha1: String,
  sum16: String,
  number: u16,
}
impl From<&XmlRomDataMisc> for RomSectionMisc {
  fn from(value: &XmlRomDataMisc) -> Self {
    Self {
      size: value.size.parse().unwrap(),
      crc32: value.crc32.clone(),
      sha1: value.sha1.clone(),
      sum16: value.sum16.clone(),
      number: value.number.parse().unwrap(),
    }
  }
}

#[repr(u8)]
#[derive(Default, Debug, Clone, Copy, serde::Serialize, serde::Deserialize, bitcode::Encode, bitcode::Decode, bincode::Encode)]
enum Mirroring {
  #[default]
  Horizontal,
  Vertical,
  SingleScreen,
  FourScreens,
}
impl From<&str> for Mirroring {
  fn from(value: &str) -> Self {
    match value {
      "H" => Self::Horizontal,
      "V" => Self::Vertical,
      "1" => Self::SingleScreen,
      "4" => Self::FourScreens,
      _ => unreachable!()
    }
  }
}
impl From<Mirroring> for nes_emulator::emu::Mirroring {
  fn from(value: Mirroring) -> Self {
    match value {
        Mirroring::FourScreens => Self::FourScreens,
        Mirroring::Horizontal => Self::Horizontal,
        Mirroring::Vertical => Self::Vertical,
        Mirroring::SingleScreen => Self::LowTable,
    }
  }
}

#[derive(Default, Debug, Clone, Copy, serde::Serialize, serde::Deserialize, bitcode::Encode, bitcode::Decode, bincode::Encode)]
enum Region {
  #[default] NTSC,
  PAL,
  Multiple,
  Other,
}
impl From<usize> for Region {
  fn from(value: usize) -> Self {
    match value {
      0 => Self::NTSC,
      1 => Self::PAL,
      2 => Self::Multiple,
      _ => Self::Other,
    }
  }
}
impl From<Region> for nes_emulator::emu::Region {
  fn from(value: Region) -> Self {
    match value {
      Region::NTSC => Self::NTSC,
      Region::PAL => Self::PAL,
      Region::Multiple => Self::World,
      Region::Other => Self::Dendy,
    }
  }
}

#[derive(Default, Debug, serde::Serialize, serde::Deserialize, bincode::Encode, bitcode::Encode, bitcode::Decode)]
struct JsonEntry {
  title: String,
  category: String,

  rom: RomSection,
  prg: RomSection,
  chr: Option<RomSection>,
  
  prgram_size: Option<usize>,
  prgnvram_size: Option<usize>,
  chrram_size: Option<usize>,
  chrnvram_size: Option<usize>,
  misc_rom: Option<RomSectionMisc>,

  mapper: usize,
  submapper: usize,
  mirroring: Mirroring,
  battery: bool,
  region: Region,
  console: u8,
  expansions: u8,
  
  vs: Option<VsSystem>,
}

#[test]
fn db_xml_to_json() {
  let db_xml = include_str!("../utils/nes20db.xml");
  let names = db_xml.lines()
    .filter(|line| line.contains("<game><!--"))
    // .inspect(|line| println!("{line}"))
    .map(|line| {
      let comps = line
        .trim()
        .strip_prefix("<game><!-- ")
        .unwrap()
        .split("\\");

      let count = comps.clone().count();
      let kind = comps.clone()
        .take(count - 1)
        .collect::<Vec<_>>()
        .join(", ");

      let name = comps.last().unwrap();
      (kind, name.strip_suffix(".nes -->").unwrap())
    })
    .collect::<Vec<_>>();

  let mut categories = HashSet::new();
  for entry in &names {
    if entry.0.contains(',') {
      let split = entry.0.split(",");
      categories.extend(split);
    } else {
      categories.insert(entry.0.as_str());
    }
  }

  println!("{categories:?}");

  let db_json = include_str!("../utils/nes20db.xml.json");
  let entries: XmlRoot = serde_json::from_str(db_json).unwrap();

  let mut json_entries = Vec::new();

  for (entry, (category, name)) in entries.nes20db.game.iter().zip(names) {  
    let json_entry = JsonEntry {
      title: name.replace("&amp;", "&"),
      category: category.replace("&amp;", "&"),
      rom: RomSection::from(&entry.rom),

      prg: RomSection::from(&entry.prgrom),
      chr: entry.chrrom.as_ref().and_then(|x| Some(x.into())),
      misc_rom: entry.miscrom.as_ref().and_then(|x| Some(x.into())),

      mapper: entry.pcb._mapper.parse().unwrap(),
      submapper: entry.pcb._submapper.parse().unwrap(),
      mirroring: Mirroring::from(entry.pcb._mirroring.as_str()),
      battery: entry.pcb._battery.parse::<u8>().unwrap() == 1,

      console: entry.console.kind.parse().unwrap(),
      region:  Region::from(entry.console.region.parse::<usize>().unwrap()),
      vs: entry.vs.as_ref().and_then(|x| Some(x.into())),

      expansions: entry.expansion.kind.parse().unwrap(),

      chrram_size: entry.chrram.as_ref().and_then(|x| Some(x._size.parse().unwrap())),
      prgram_size: entry.prgram.as_ref().and_then(|x| Some(x._size.parse().unwrap())),
      chrnvram_size: entry.chrnvram.as_ref().and_then(|x| Some(x._size.parse().unwrap())),
      prgnvram_size: entry.prgnvram.as_ref().and_then(|x| Some(x._size.parse().unwrap())),
    };

    json_entries.push(json_entry);
  }

  _ = std::fs::create_dir("./db_tests");
  let file = std::fs::File::create("./db_tests/nes20db.json").unwrap();
  let buf = std::io::BufWriter::new(file);
  serde_json::to_writer_pretty(buf, &json_entries).unwrap()
}


impl From<&JsonEntry> for GameData {
  fn from(value: &JsonEntry) -> Self {
    Self {
      prg_size: value.prg.size,
      chr_size: value.chr.clone().and_then(|it| Some(it.size)).unwrap_or_default(),
      
      title: value.title.clone(),
      rom_total_size: value.rom.size,
      // rom_crc32: value.rom.crc32.clone(),
      // rom_sum16: value.rom.sum16.clone(),
      // rom_sha1: value.rom.sha1.clone(),
      // rom_sha1: sha1_str_to_arr(&value.rom.sha1),
      rom_crc32: crc32_str_to_int(&value.rom.crc32),

      // prg_crc32: value.prg.crc32.clone(),
      // prg_sum16: value.prg.sum16.clone(),
      // prg_sha1: value.prg.sha1.clone(),
      // prg_sha1: sha1_str_to_arr(&value.prg.sha1),
      prg_crc32: crc32_str_to_int(&value.prg.crc32),

      prgram_size: value.prgram_size.unwrap_or_default(),
      prgnvram_size: value.prgnvram_size.unwrap_or_default(),
      chrram_size: value.chrram_size.unwrap_or_default(),
      // chrnvram_size: value.chrnvram_size.unwrap_or_default(),
      mapper: value.mapper as u16,
      submapper: value.submapper as u8,
      mirroring: value.mirroring.into(),
      has_battery: value.battery,
      region: value.region.into(),
      console: value.console,
      expansions: value.expansions,
    }
  }
}

#[test]
fn compress_db() {
  let file = include_str!("../utils/nes20db.json");
  let json_entries: Vec<JsonEntry> = serde_json::from_str(file).unwrap();

  let categories = json_entries.iter()
    .map(|x| x.category.clone())
    .collect::<HashSet<_>>();
  println!("{categories:?}");

  let lite = json_entries.iter()
    .map(|x| GameData::from(x))
    .collect::<Vec<_>>();

  _ = fs::create_dir("./db_tests");

  let create_file = |path: &str| {
    let file = std::fs::File::create(path).unwrap();
    std::io::BufWriter::new(file)
  };

  // let out = create_file(".cbor");
  // ciborium::into_writer(&json_entries, out).unwrap();
  // let mut out = create_file(".msgpack");
  // rmp_serde::encode::write(&mut out, &json_entries).unwrap();

  // let mut out = create_file(".bincode");
  // let bin = bincode::encode_to_vec(&json_entries, bincode::config::standard()).unwrap();
  // out.write_all(&bin).unwrap();
  // let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
  // gzip.write_all(&bin).unwrap();
  // let gzip = gzip.finish().unwrap();
  // let mut out = create_file(".bincode.gzip");
  // out.write_all(&gzip).unwrap();

  // bitcode is the best

  let mut out = create_file("./db_tests/nes20db.json.bitcode");
  let bin = bitcode::encode(&json_entries);
  out.write_all(&bin).unwrap();

  let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
  gzip.write_all(&bin).unwrap();
  let gzip = gzip.finish().unwrap();
  let mut out = create_file("./db_tests/nes20db.json.bitcode.gzip");
  out.write_all(&gzip).unwrap();

  let mut out = create_file("./db_tests/nes20db.bitcode");
  let bin = bitcode::encode(&lite);
  out.write_all(&bin).unwrap();

  let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
  gzip.write_all(&bin).unwrap();
  let gzip = gzip.finish().unwrap();
  let mut out = create_file("./utils/nes20db.bitcode.gzip");
  out.write_all(&gzip).unwrap();
}

#[test]
fn decode_db() {
  let file = include_bytes!("../utils/nes20db.bitcode.gzip").as_slice();

  let mut decode = flate2::read::GzDecoder::new(file);
  let mut buf = Vec::new();
  decode.read_to_end(&mut buf).unwrap();

  let parsed: Vec<GameData> = bitcode::decode(&buf).unwrap();
  println!("{:#?}", &parsed[0..10]);
}

#[test]
fn crc32_count() {
  let file = include_str!("../utils/nes20db.json");
  let json_entries: Vec<JsonEntry> = serde_json::from_str(file).unwrap();

  let mut rom_map: HashMap<_, usize> = HashMap::new();
  let mut prg_map: HashMap<_, usize> = HashMap::new();
  json_entries.iter()
    .map(|x| (x.rom.crc32.clone(), x.prg.crc32.clone()))
    .for_each(|x| {
      rom_map.entry(x.0).and_modify(|e| *e += 1).or_default();
      prg_map.entry(x.1).and_modify(|e| *e += 1).or_default();
    });

  let rom_same = rom_map.values().filter(|x| **x > 0).count();
  let prg_same = prg_map.values().filter(|x| **x > 0).count();
  
  dbg!(rom_same);
  dbg!(prg_same);

  let same = prg_map.iter().filter(|(_, x)| **x > 0).collect::<Vec<_>>();
  println!("{same:?}");
}