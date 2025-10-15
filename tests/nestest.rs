use nes_emulator::{cpu::Status, emu::{self, Emu}};

#[derive(Debug, Default, PartialEq, Eq)]
struct LogLine {
  pc: u16,
  a: u8,
  x: u8,
  y: u8,
  sp: u8,
  p: Status,

  ppu_cycles: usize,
  scanlines: usize,
  cpu_cycles: usize,
}

fn parse_logline(line: &str) -> LogLine {
  let mut tokens = line.split_whitespace();
  let mut logline = LogLine::default();

  logline.pc = u16::from_str_radix(tokens.next().unwrap(), 16).unwrap();
  let mut tokens = tokens.rev();

  logline.cpu_cycles = tokens.next().unwrap().split_once(':').unwrap().1.parse().unwrap();
  logline.ppu_cycles = tokens.next().unwrap().parse().unwrap();
  logline.scanlines = tokens.next().unwrap().strip_suffix(",").unwrap().parse().unwrap();
  
  logline.sp = u8::from_str_radix(tokens.nth(1).unwrap().split_once(':').unwrap().1, 16).unwrap();
  logline.p = Status::from_bits_retain(u8::from_str_radix(tokens.next().unwrap().split_once(':').unwrap().1, 16).unwrap());
  logline.y = u8::from_str_radix(tokens.next().unwrap().split_once(':').unwrap().1, 16).unwrap();
  logline.x = u8::from_str_radix(tokens.next().unwrap().split_once(':').unwrap().1, 16).unwrap();
  logline.a = u8::from_str_radix(tokens.next().unwrap().split_once(':').unwrap().1, 16).unwrap();

  logline
}

fn logline_from_emu(emu: &Emu) -> LogLine {
  let mut logline = LogLine::default();
  logline.pc = emu.cpu.pc;
  logline.a = emu.cpu.a;
  logline.x = emu.cpu.x;
  logline.y = emu.cpu.y;
  logline.p = emu.cpu.p;
  logline.sp = emu.cpu.sp;

  logline.ppu_cycles = emu.ppu.dots as usize;
  logline.scanlines = emu.ppu.scanline as usize;
  logline.cpu_cycles = emu.cpu.cycles;

  logline
}

use pretty_assertions::assert_eq;

#[test]
fn nestest_no_graphics() {
  let mut emu = Emu::load_rom_from_bytes(include_bytes!("../roms/nestest.nes"), None).unwrap();
  emu.cpu.pc = 0xc000;
  emu.cpu.cycles =  7;
  emu.ppu.dots  = 21;
  emu.ppu.scanline = 0;
  // emu.cpu.p = Status::IrqDisable | Status::Unused;

  let log = include_str!("./nestest.log");

  let mut cycles = 0;
  log.lines().enumerate().for_each(|(i, line)| {
    let good = parse_logline(line);
    let mine = logline_from_emu(&emu);
    
    assert_eq!(good, mine, "(log == mine) [Wrong line = {}]\nLast op cycles = {}\n{}", 
    i+1, emu.cpu.cycles - cycles, line);
    // println!("Line {} OK", i+1);
    
    cycles = emu.cpu.cycles;
    emu.cpu_step();
  });

  println!("Result: {} {}", emu.cpu_dispatch_read(0x2), emu.cpu_dispatch_read(0x3));
}