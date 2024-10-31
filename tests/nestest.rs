#![allow(unused_imports)]

#[cfg(test)]
mod tests {
  use core::panic;
  use std::{collections::VecDeque, fs, path::Path};
  use circular_buffer::CircularBuffer;
use log::info;

  use nen_emulator::emu::{cart::Cart, cpu::{Cpu, CpuFlags, STACK_START}, instr::{AddressingMode, INSTRUCTIONS}};
use prettydiff::{diff_lines, diff_words};

  #[derive(Eq)]
  struct CpuMock {
    pc: u16,
    sp: u8,
    a: u8,
    x: u8,
    y: u8,
    p: u8,
    cycles: usize
  }
  impl PartialEq for CpuMock {
    fn eq(&self, other: &Self) -> bool {
        self.pc == other.pc && self.sp == other.sp && self.a == other.a && self.x == other.x && self.y == other.y && self.p == other.p 
        //&& self.cycles == other.cycles
    }
  }

  impl CpuMock {
    fn from_cpu(cpu: &Cpu) -> Self {
      CpuMock {pc: cpu.pc, sp: cpu.sp, a: cpu.a, x: cpu.x, y: cpu.y, p: cpu.p.bits(), cycles: cpu.cycles }
    }

    fn from_log(line: &str) -> Self {
      let mut tokens = line.split_whitespace();
      
      let pc = u16::from_str_radix(tokens.next().unwrap(), 16).unwrap();
  
      let mut tokens = tokens.rev();
      let cycles = usize::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 10).unwrap();
      let mut tokens = tokens.skip_while(|tok| !tok.contains("SP"));
  
      let sp = u8::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 16).unwrap();
      let p = u8::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 16).unwrap();
      let y = u8::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 16).unwrap();
      let x = u8::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 16).unwrap();
      let a = u8::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 16).unwrap();
  
      CpuMock {
        pc, a, x, y, sp, p, cycles,
      }
    }
  }

  #[test]
  #[ignore]
  fn open_rom() {
    let rom_path = Path::new("tests/nestest.nes");
    let cart = Cart::new(rom_path);
    println!("{:?}", cart.header);
  }

  fn debug_line(cpu: &CpuMock, mem: &[u8]) -> String {
    let opcode = &INSTRUCTIONS[mem[cpu.pc as usize] as usize]; 
    
    let operand8 = mem[cpu.pc as usize+1];
    let operand16 = u16::from_le_bytes([operand8, mem[cpu.pc as usize+2]]);

    use AddressingMode::*;
    let desc = match opcode.addressing {
      Implicit | Accumulator => String::new(),
      Immediate => format!("#${operand8:02X}"),
      Relative => format!("${:04X}", (cpu.pc+opcode.bytes as u16).wrapping_add_signed((operand8 as i8) as i16)),
      ZeroPage | ZeroPageX | ZeroPageY => format!("${operand8:02X} = ${:04X}", mem[operand8 as usize]),
      Absolute => format!("${operand16:04X} = ${:02X}", mem[operand16 as usize]),
      AbsoluteX => format!("${:04X} = ${:02X}", operand16+cpu.x as u16, mem[(operand16+cpu.x as u16) as usize]),
      AbsoluteY => format!("${:04X} = ${:02X}", operand16+cpu.y as u16, mem[(operand16+cpu.y as u16) as usize]),
      Indirect => format!("${operand16:04X} = {:04X}", mem[operand16 as usize]),
      IndirectX => format!("IndX ${:04X} @ {:04X}", operand8+cpu.x, mem[(operand8+cpu.x) as usize]),
      IndirectY => format!("IndY ${:04X} @ {:04X}", operand8, mem[operand8 as usize] as u16 + cpu.y as u16),
    };

    format!(
      "{:04X}  {:02X} {:02X} {:02X}  {opcode} {desc:20} \
      A:{a:02X} X:{x:02X} Y:{y:02X} P:{p:02X} SP:{sp:02X} CYC:{cyc}",
      cpu.pc, mem[cpu.pc as usize], operand8, mem[cpu.pc as usize+2],
      opcode=opcode.name,
      a=cpu.a, x=cpu.x, y=cpu.y, p=cpu.p, sp=cpu.sp, cyc=cpu.cycles
    )
  }

  #[test]
  fn nes_test() {
    let mut builder = colog::basic_builder();
    builder.filter_level(log::LevelFilter::Info);
    builder.init();

    let mut test_log = include_str!("nestest.log")
      .lines().enumerate();

    let rom_path = Path::new("tests/nestest.nes");
    let prg_rom = fs::read(rom_path).unwrap();
    let mut cpu = Cpu::new();

    cpu.pc = 0xC000;
    cpu.p = CpuFlags::from_bits_retain(0x24);
    cpu.write_data(0x8000, &prg_rom[16..16+0x4000]);
    cpu.write_data(0xC000, &prg_rom[16..16+0x4000]);
    
    const RANGE: usize = 8;
    let mut most_recent_instr = CircularBuffer::<RANGE, (CpuMock, CpuMock)>::new();

    cpu.interpret_with_callback(move |cpu| {
      let (mut line_count, line) = test_log.next().unwrap();
      line_count+=1;
      
      let my_cpu = CpuMock::from_cpu(cpu);
      let log_cpu = CpuMock::from_log(line);

      if my_cpu != log_cpu {
        let mut trace = most_recent_instr.iter().enumerate().collect::<Vec<_>>();
        trace.sort_by(|a, b| a.0.cmp(&b.0));

        for (i, (mine, log)) in trace {
          let my_line = debug_line(mine, cpu.mem.borrow().as_slice());
          let log_line = debug_line(log, cpu.mem.borrow().as_slice());
    
          let line = line_count - RANGE + i;
          info!("{}|Mine -> {my_line}", line); 
          info!("{}|Log  -> {log_line}", line);
          info!("");
        }

        let my_line = debug_line(&my_cpu, cpu.mem.borrow().as_slice());
        let log_line = debug_line(&log_cpu, cpu.mem.borrow().as_slice());
        info!("{}|Mine -> {my_line}", line_count); 
        info!("{}|Log  -> {log_line}", line_count);
        info!("");

        info!("{}", "-".repeat(50));
        info!("Incosistency at line {line_count}\n{}", diff_words(&my_line, &log_line));

        let my_p = format!("{:?}", CpuFlags::from_bits_retain(my_cpu.p));
        let log_p = format!("{:?}", CpuFlags::from_bits_retain(log_cpu.p));
        info!("Stack: {}", cpu.stack_trace());

        info!("Flags: {}", diff_lines(&my_p, &log_p));
        info!("Results: ${:04X}", cpu.mem_fetch16(0x2));

        info!("{}", "-".repeat(50));
        panic!()
      }

      most_recent_instr.push_back((my_cpu, log_cpu));
    });
  }
}