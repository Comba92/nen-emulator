#[cfg(test)]
pub mod nes_test {
use core::panic;
use std::{path::Path, rc::Rc};
use circular_buffer::CircularBuffer;
use log::info;

use nen_emulator::emu::{bus::Bus, cart::Cart, cpu::{Cpu, CpuFlags}, instr::{AddressingMode, INSTRUCTIONS}, ppu::Ppu, Emulator};
use prettydiff::{diff_lines, diff_words};

  #[derive(Debug, PartialEq, Eq, Clone)]
  struct CpuMock {
    pc: u16,
    sp: u8,
    a: u8,
    x: u8,
    y: u8,
    p: u8,
    scanlines: usize,
    ppu_cycles: usize,
    cpu_cycles: usize
  }
  // impl PartialEq for CpuMock {
  //   fn eq(&self, other: &Self) -> bool {
  //       self.pc == other.pc && self.sp == other.sp && self.a == other.a && self.x == other.x && self.y == other.y && self.p == other.p 
  //       && self.cpu_cycles == other.cpu_cycles
  //       && self.ppu_cycles == other.ppu_cycles
  //       && self.scanlines == other.scanlines
  //   }
  // }

  impl CpuMock {
    fn from_cpu(cpu: &Cpu, ppu: &Ppu) -> Self {
      CpuMock {
        pc: cpu.pc, sp: cpu.sp, a: cpu.a, x: cpu.x, y: cpu.y, p: cpu.p.bits(), 
        cpu_cycles: cpu.cycles,
        scanlines: ppu.scanline,
        ppu_cycles: ppu.cycles,
      }
    }

    fn from_log(line: &str) -> Self {
      let mut tokens = line.split_whitespace();
      
      let pc = u16::from_str_radix(tokens.next().unwrap(), 16).unwrap();
  
      let mut tokens = tokens.skip_while(|token| !token.contains("A:"));
          
      let a = u8::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 16).unwrap();
      let x = u8::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 16).unwrap();
      let y = u8::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 16).unwrap();
      let p = u8::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 16).unwrap();
      let sp = u8::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 16).unwrap();
      
      let ppu_data = tokens.clone()
        .take_while(|token| !token.contains("CYC:"))
        .collect::<String>();

      let scanlines = usize::from_str_radix(ppu_data.split(',').nth(0).unwrap().trim_start_matches("PPU:").trim(), 10).unwrap();
      let ppu_cycles = usize::from_str_radix(ppu_data.split(',').nth(1).unwrap().trim(), 10).unwrap();
      let cpu_cycles = usize::from_str_radix(tokens.skip_while(|token| !token.contains("CYC:")).next().unwrap().split(':').last().unwrap(), 10).unwrap();
      
      CpuMock {
        pc, a, x, y, sp, p, cpu_cycles, ppu_cycles, scanlines
      }
    }
  }

  #[test]
  fn parse_log_line() {
    let log_str = include_str!("nestest.log");
    let mut test_log = log_str
      .lines();

    let cpu = CpuMock::from_log(test_log.next().unwrap());
    println!("{:?}", cpu);
  }

  #[test]
  fn open_rom() {
    let rom_path = Path::new("tests/test_roms/Donkey Kong.nes");
    let cart = Cart::new(rom_path);
    println!("{:?}", cart.header);
  }

  fn debug_line(cpu: &CpuMock, bus: &Rc<Bus>) -> String {
    let opcode = bus.read(cpu.pc);
    let instr = &INSTRUCTIONS[opcode as usize];
    
    let operand8 = bus.read(cpu.pc+1);
    let operand16 = u16::from_le_bytes([operand8, bus.read(cpu.pc+2)]);

    use AddressingMode::*;
    let desc = match instr.addressing {
      Implicit | Accumulator => String::new(),
      Immediate => format!("#${operand8:02X}"),
      Relative => format!("${:04X}", (cpu.pc+instr.bytes as u16).wrapping_add_signed((operand8 as i8) as i16)),
      ZeroPage | ZeroPageX | ZeroPageY => format!("${operand8:02X} = ${:04X}", bus.read(operand8 as u16)),
      Absolute => format!("${operand16:04X} = ${:02X}", bus.read(operand16)),
      AbsoluteX => format!("${:04X} = ${:02X}", operand16+cpu.x as u16, bus.read(operand16+cpu.x as u16)),
      AbsoluteY => format!("${:04X} = ${:02X}", operand16+cpu.y as u16, bus.read(operand16+cpu.y as u16)),
      Indirect => format!("${operand16:04X} = {:04X}", bus.read(operand16)),
      IndirectX => format!("IndX ${:04X} @ {:04X}", operand8+cpu.x, bus.read((operand8+cpu.x) as u16)),
      IndirectY => format!("IndY ${:04X} @ {:04X}", operand8, bus.read((operand8 as u16) as u16 + cpu.y as u16)),
    };

    format!(
      "{:04X}  {:02X} {:02X} {:02X}  {instr} {desc:20} \
      A:{a:02X} X:{x:02X} Y:{y:02X} P:{p:02X} SP:{sp:02X} \
      PPU: {scanline}, {pixel} CYC:{cyc}",
      cpu.pc, opcode, operand8, bus.read(cpu.pc + 2),
      instr=instr.name, scanline=cpu.scanlines, pixel=cpu.ppu_cycles,
      a=cpu.a, x=cpu.x, y=cpu.y, p=cpu.p, sp=cpu.sp, cyc=cpu.cpu_cycles
    )
  }

  const LINES_RANGE: usize = 8;

  #[test]
  fn nes_test() {
    let mut builder = colog::basic_builder();
    builder.filter_level(log::LevelFilter::Info);
    builder.init();

    let log_str = include_str!("nestest.log");
    let mut test_log = log_str
      .lines();

    let rom_path = Path::new("tests/nestest/nestest.nes");
    let rom = Cart::new(rom_path);
    let mut emu = Emulator::from_cart(rom);

    emu.cpu.pc = 0xC000;
    emu.cpu.p = CpuFlags::from_bits_retain(0x24);
    emu.bus.write_data(0x8000, &emu.cart.prg_rom[..0x4000]);
    emu.bus.write_data(0xC000, &emu.cart.prg_rom[..0x4000]);
    
    let mut most_recent_instr = CircularBuffer::<LINES_RANGE, (CpuMock, CpuMock)>::new();
    let mut line_count = 1;

    loop {
      let next_line = test_log.next();
      
      if let None = next_line {
        info!("Reached end of input!!");
        print_last_diffs(&most_recent_instr, &emu.cpu, line_count);
        info!("Errors: ${:02X}", &emu.cpu.mem_read(0x2));
        info!("Results: ${:04X}", &emu.cpu.mem_read16(0x2));
        
        // let mut builder = colog::basic_builder();
        // builder.filter_level(log::LevelFilter::Info);
        // builder.init();
        // for _ in 0..100 {
        //   emu.step();
        // }
        
        break;
      }
      
      let line = next_line.unwrap();
      let my_cpu = CpuMock::from_cpu(&emu.cpu, &emu.bus.ppu());
      let log_cpu = CpuMock::from_log(line);
      
      if my_cpu != log_cpu {
        print_last_diffs(&most_recent_instr, &emu.cpu, line_count);
        
        let my_line = debug_line(&my_cpu, &emu.cpu.bus);
        let log_line = debug_line(&log_cpu, &emu.cpu.bus);
        info!("{}|Mine -> {my_line}", line_count);
        info!("{}|Log  -> {log_line}", line_count);
        info!("");
        
        info!("{}", "-".repeat(50));
        info!("Incosistency at line {line_count}\n{}", diff_words(&my_line, &log_line));
        
        let my_p = format!("{:?}", CpuFlags::from_bits_retain(my_cpu.p));
        let log_p = format!("{:?}", CpuFlags::from_bits_retain(log_cpu.p));
        info!("Stack: {}", &emu.cpu.stack_trace());
        
        info!("Flags: {}", diff_lines(&my_p, &log_p));
        info!("Results: ${:04X}", &emu.cpu.mem_read16(0x2));
        
        info!("{}", "-".repeat(50));
        
        panic!("Instruction inconsistency")
      }
      
      most_recent_instr.push_back((my_cpu, log_cpu));
      line_count+=1;
      emu.step();
    }
  }
  
  
  fn print_last_diffs(most_recent_instr: &CircularBuffer<8, (CpuMock, CpuMock)>, cpu: &Cpu, line_count: usize) {
    let mut trace: Vec<(usize, &(CpuMock, CpuMock))> = most_recent_instr.iter().enumerate().collect::<Vec<_>>();
    trace.sort_by(|a, b| a.0.cmp(&b.0));
    
    for (i, (mine, log)) in trace {
      let my_line = debug_line(mine, &cpu.bus);
      let log_line = debug_line(log, &cpu.bus);
    
      let line = line_count.max(LINES_RANGE) - LINES_RANGE + i;
      info!("{}|Mine -> {my_line}", line); 
      info!("{}|Log  -> {log_line}", line);
      info!("");
    }
  }
}