#[cfg(test)]
mod tests {
    use std::{cell::RefCell, fs, path::Path, rc::Rc};
    use nen_emulator::emu::{cart::Cart, cpu::{interpret, interpret_with_callback, Cpu, StatusReg}, instr::{INSTRUCTIONS, OPCODES_MAP}};

  struct CpuMock {
    ip: u16,
    sp: u8,
    a: u8,
    x: u8,
    y: u8,
    sr: u8,
    cycles: usize
  }
  impl CpuMock {
    fn from_cpu(cpu: &Cpu) -> Self {
      CpuMock {ip: cpu.ip, sp: cpu.sp, a: cpu.a, x: cpu.x, y: cpu.y, sr: cpu.sr.bits(), cycles: cpu.cycles }
    }
  }

  #[test]
  fn open_rom() {
    let rom_path = Path::new("tests/nestest.nes");
    let cart = Cart::new(rom_path);
    println!("{:?}", cart.header);
  }

  fn debug_line(cpu: CpuMock, mem: &[u8]) {
    let opcode = &INSTRUCTIONS[mem[cpu.ip as usize] as usize]; 
    
    println!(
      "{:04X}  {:02X} {:02X} {:02X}  {opcode}\t\t \
      A:{a:02X} X:{x:02X} Y:{y:02X} P:{sp:02X} SP:{sr:02X} CYC:{cyc}",
      cpu.ip, mem[cpu.ip as usize], mem[cpu.ip as usize+1], mem[cpu.ip as usize+2],
      opcode=opcode.name,
      a=cpu.a, x=cpu.x, y=cpu.y, sp=cpu.sp, sr=cpu.sr, cyc=cpu.cycles
    );
  }

  fn parse_log_line(line: &str) -> CpuMock {
    let mut tokens = line.split_whitespace();
    
    let ip = u16::from_str_radix(tokens.next().unwrap(), 16).unwrap();

    let mut tokens = tokens.rev();
    let cycles = usize::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 10).unwrap();
    let mut tokens = tokens.skip_while(|tok| !tok.contains("SP"));

    let sr = u8::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 16).unwrap();
    let sp = u8::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 16).unwrap();
    let y = u8::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 16).unwrap();
    let x = u8::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 16).unwrap();
    let a = u8::from_str_radix(tokens.next().unwrap().split(':').last().unwrap(), 16).unwrap();

    CpuMock {
      ip, a, x, y, sp, sr, cycles,
    }
  }

  #[test]
  fn nes_test() {
    let mut test_log = include_str!("nestest.log").lines();

    let rom_path = Path::new("tests/nestest.nes");
    let prg_rom = fs::read(rom_path).unwrap();
    let mut cpu = Cpu::new();

    cpu.ip = 0xC000;
    cpu.write_data(0x8000, &prg_rom[16..16+0x4000]);
    cpu.write_data(0xC000, &prg_rom[16..16+0x4000]);
    
    println!("Starting interpreter...");
    interpret_with_callback(&mut cpu, move |cpu| {
      print!("Mine -> "); debug_line(CpuMock::from_cpu(cpu), cpu.mem.borrow().as_slice());
      let log_cpu = parse_log_line(test_log.next().unwrap());
      print!("Log  -> "); debug_line(log_cpu, cpu.mem.borrow().as_slice());
      println!();
    });
  }
}