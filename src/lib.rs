pub mod emu;
pub mod cpu;
mod bus;
pub mod cart;
mod mapper;

// mod utils {
//   pub fn bit_get(x: u8, bit: u8) -> bool { (x >> bit) & 1 == 1 }
//   pub fn bit_set(x: u8, flags: u8) -> u8 { x | flags }
//   pub fn bit_set_with(x: u8, flags: u8, cond: bool) -> u8 {
//     if cond { bit_set(x, flags) }
//     else { bit_clear(x, flags) }
//   }
//   pub fn bit_clear(x: u8, flags: u8) -> u8 { x & !flags }
//   pub fn bit_toggle(x: u8, flags: u8) -> u8 { x ^ flags }
// }