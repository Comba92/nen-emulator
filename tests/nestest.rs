use nes_emulator::{cart::Cart, emu::Emu};

#[test]
fn nestest_no_graphics() {
  let rom = Cart::new(include_bytes!("../roms/nestest.nes"))
    .unwrap();

  println!("{:?}", rom.header);
  
  let mut emu = Emu::new(rom);
  emu.cpu.pc = 0xc000;

  // run for 90000 instructions
  loop {
    emu.step();
    if emu.cpu.pc == 0x8991 { break; }
  }

  println!("{} {}", emu.read8(0x2), emu.read8(0x3));
}