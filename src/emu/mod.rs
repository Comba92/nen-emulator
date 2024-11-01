use cpu::Cpu;
use ppu::Ppu;
use bus::Bus;

pub mod cpu;
pub mod ppu;
pub mod bus;

pub mod instr;
pub mod cart;

pub mod ui;

pub struct Emulator {
  cpu: Cpu,
  bus: Bus,
  ppu: Ppu,
}