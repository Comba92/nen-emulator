#[cfg(test)]
mod tests {
  use core::panic;
  use std::{fs, path::Path};
  use nen_emulator::emu::{cart::Cart, cpu::{interpret_with_callback, Cpu, Status}, instr::{AddressingMode, INSTRUCTIONS}};
  use prettydiff::diff_words;

  #[derive(PartialEq, Eq)]
  struct CpuMock {
    ip: u16,
    sp: u8,
    a: u8,
    x: u8,
    y: u8,
    p: u8,
    cycles: usize
  }
  impl CpuMock {
    fn from_cpu(cpu: &Cpu) -> Self {
      CpuMock {ip: cpu.pc, sp: cpu.sp, a: cpu.a, x: cpu.x, y: cpu.y, p: cpu.p.bits(), cycles: cpu.cycles }
    }

    fn from_log(line: &str) -> Self {
      let mut tokens = line.split_whitespace();
      
      let ip = u16::from_str_radix(tokens.next().unwrap(), 16).unwrap();
  
      let mut tokens = tokens.rev();
      let cycles = usize::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 10).unwrap();
      let mut tokens = tokens.skip_while(|tok| !tok.contains("SP"));
  
      let sp = u8::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 16).unwrap();
      let p = u8::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 16).unwrap();
      let y = u8::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 16).unwrap();
      let x = u8::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 16).unwrap();
      let a = u8::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 16).unwrap();
  
      CpuMock {
        ip, a, x, y, sp, p, cycles,
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
    let opcode = &INSTRUCTIONS[mem[cpu.ip as usize] as usize]; 
    
    let operand8 = mem[cpu.ip as usize+1];
    let operand16 = u16::from_le_bytes([operand8, mem[cpu.ip as usize+2]]);

    use AddressingMode::*;
    let desc = match opcode.addressing {
      Implicit | Accumulator => String::new(),
      Immediate => format!("#${operand8:02X}"),
      Relative => format!("${:04X}", (cpu.ip+opcode.bytes as u16).wrapping_add_signed((operand8 as i8) as i16)),
      ZeroPage | ZeroPageX | ZeroPageY => format!("${operand8:02X} = ${:02X}", mem[operand8 as usize]),
      Absolute | AbsoluteX | AbsoluteY => format!("${operand16:04X} = ${:02X}", mem[operand16 as usize]),
      Indirect | IndirectX | IndirectY => format!("${operand16:04X} = {:04X}", mem[operand16 as usize]),
    };

    format!(
      "{:04X}  {:02X} {:02X} {:02X}  {opcode} {desc:20} \
      A:{a:02X} X:{x:02X} Y:{y:02X} P:{p:02X} SP:{sp:02X} CYC:{cyc}",
      cpu.ip, mem[cpu.ip as usize], operand8, mem[cpu.ip as usize+2],
      opcode=opcode.name,
      a=cpu.a, x=cpu.x, y=cpu.y, p=cpu.p, sp=cpu.sp, cyc=cpu.cycles
    )
  }

  fn debug_flags(cpu: CpuMock) {
    println!("Flags set:");
    let flags = Status::from_bits_retain(cpu.p);
    for (name, _) in flags.iter_names() {
      println!("{name}");
    }
    println!()
  }

  #[test]
  fn nes_test() {
    let mut test_log = include_str!("nestest.log")
      .lines().enumerate();

    let rom_path = Path::new("tests/nestest.nes");
    let prg_rom = fs::read(rom_path).unwrap();
    let mut cpu = Cpu::new();

    cpu.pc = 0xC000;
    cpu.p = Status::from_bits_retain(0x24);
    cpu.write_data(0x8000, &prg_rom[16..16+0x4000]);
    cpu.write_data(0xC000, &prg_rom[16..16+0x4000]);
    
    println!("Starting interpreter...");
    interpret_with_callback(&mut cpu, move |cpu| {
      let (mut line_count, line) = test_log.next().unwrap();
      line_count+=1;
      
      let my_cpu = CpuMock::from_cpu(cpu);
      let log_cpu = CpuMock::from_log(line);
      
      let my_line = debug_line(&my_cpu, cpu.mem.borrow().as_slice());
      let log_line = debug_line(&log_cpu, cpu.mem.borrow().as_slice());

      println!("{line_count}|Mine -> {my_line}"); 
      println!("{line_count}|Log  -> {log_line}");
      println!();

      if my_cpu != log_cpu {
        println!("{}", "-".repeat(50));
        println!("Incosistency at line {line_count}:\n{}", diff_words(&my_line, &log_line));
        debug_flags(my_cpu);
        debug_flags(log_cpu);
        println!("{}", "-".repeat(50));
        panic!()
      }
      
    });
  }
}