use crate::{cart::{ConsoleTiming, SharedCart}, frame::FrameBuffer};
use bitfield_struct::bitfield;
use bitflags::bitflags;
use render::Fetcher;

mod render;

bitflags! {
	#[derive(Default, serde::Serialize, serde::Deserialize)]
	struct Ctrl: u8 {
		const base_nametbl = 0b0000_0011;
		const vram_incr    = 0b0000_0100;
		const spr_ptrntbl  = 0b0000_1000;

		const bg_ptrntbl   = 0b0001_0000;
		const spr_big      = 0b0010_0000;
		const master_slave = 0b0100_0000;
		const nmi_enabled  = 0b1000_0000;
	}

	#[derive(Default, serde::Serialize, serde::Deserialize)]
	struct Mask: u8 {
		const greyscale      = 0b0000_0001;
		const bg_strip_show  = 0b0000_0010;
		const spr_strip_show = 0b0000_0100;
		const bg_enabled     = 0b0000_1000;

		const spr_enabled = 0b0001_0000;
		const red_boost   = 0b0010_0000;
		const blue_boost  = 0b0100_0000;
		const green_boost = 0b1000_0000;
	}

	#[derive(Default, serde::Serialize, serde::Deserialize)]
	struct Stat: u8 {
		const open_bus     = 0b0001_1111;
		const spr_overflow = 0b0010_0000;
		const spr0_hit     = 0b0100_0000;
		const vblank       = 0b1000_0000;
	}
}

impl Ctrl {
	pub fn vram_addr_incr(&self) -> u16 {
		match self.contains(Ctrl::vram_incr) {
				false => 1,
				true => 32,
		}
	}

	pub fn spr_ptrntbl_addr(&self) -> u16 {
		self.contains(Ctrl::spr_ptrntbl) as u16 * 0x1000
	}

	pub fn bg_ptrntbl_addr(&self) -> u16 {
		self.contains(Ctrl::bg_ptrntbl) as u16 * 0x1000
	}
	
	pub fn spr_height(&self) -> usize {
		match self.contains(Ctrl::spr_big) {
				false => 8,
				true => 16,
		}
	}
}

// https://www.nesdev.org/wiki/PPU_scrolling#PPU_internal_registers
#[bitfield(u16, order = Lsb)]
struct LoopyReg {
	#[bits(5)]
	coarse_x: u8,
	#[bits(5)]
	coarse_y: u8,
	#[bits(1)]
	nametbl_x: u8,
	#[bits(1)]
	nametbl_y: u8,
	#[bits(3)]
	fine_y: u8,
	#[bits(1)]
	__: u8,
}
impl LoopyReg {
	pub fn nametbl(&self) -> u8 {
		(self.nametbl_y() << 1) | self.nametbl_x()
	}

	pub fn nametbl_idx(&self) -> u16 {
		((self.nametbl() as u16) << 10) | ((self.coarse_y() as u16) << 5) | (self.coarse_x() as u16)
	}
}
impl serde::Serialize for LoopyReg {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer {
		serializer.serialize_u16(self.0)
	}
}
impl<'de> serde::Deserialize<'de> for LoopyReg {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de> {
		let val = u16::deserialize(deserializer)?;
		Ok(LoopyReg::from_bits(val))
	}
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
enum WriteLatch {
	#[default] FirstWrite,
	SecondWrite,
}

enum VramDst {
	Patterntbl,
	Nametbl,
	Palettes,
	Unused,
}

#[derive(Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PpuState {
	FetchBg,
	FetchSpr,
	#[default] Vblank
}

pub const NAMETABLES: u16 = 0x2000;
pub const ATTRIBUTES: u16 = 0x23C0;
pub const PALETTES: u16 = 0x3F00;

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Ppu {
	#[serde(skip)]
	pub screen: FrameBuffer,
	renderer: Fetcher,

	v: LoopyReg,   // current vram address
	t: LoopyReg,   // temporary vram address / topleft onscreen tile
	x: u8,         // Fine X Scroll
	w: WriteLatch, // First or second write toggle
	
	ctrl: Ctrl,
	mask: Mask,
	mask_tmp: u8,
	mask_write_delay: u8,
	stat: Stat,
	oam_addr: u8,
	data_buf: u8,
	
  #[serde(skip)]
	cart: SharedCart,

	palettes: [u8; 32],
	oam: Box<[u8]>,
	pub oam_sprite_limit: u8,
	
	pub scanline: usize,
	pub last_scanline: usize,
	pub cycle: usize,
	in_odd_frame: bool,
	
	pub nmi_tmp: Option<()>,
	pub nmi_requested: Option<()>,
	vblank_suppress: bool,
	nmi_suppress: bool,
	pub frame_ready: Option<()>,
}

impl Ppu {
	pub fn new(cart: SharedCart) -> Self {
		let last_scanline = 241 + cart.as_ref().header.timing.vblank_len();
		
		Self {
			screen: FrameBuffer::nes_screen(),
			renderer: Fetcher::new(),

			v: LoopyReg::new(),
			t: LoopyReg::new(),
			w: WriteLatch::FirstWrite,

			cart,
			palettes: [0; 32],
			oam: vec![0; 256].into_boxed_slice(),
			oam_sprite_limit: u8::MAX,


			scanline: last_scanline,
			last_scanline,
			
			..Default::default()
		}
	}

	pub fn wire_cart(&mut self, cart: SharedCart) {
		self.cart = cart;
	}
	
	pub fn reset(&mut self) {
		self.ctrl = Ctrl::from_bits_truncate(0);
		self.mask = Mask::from_bits_truncate(0);
		self.w = WriteLatch::FirstWrite;
		self.t.set_coarse_x(0);
		self.t.set_coarse_y(0);
		self.t.set_fine_y(0);
		self.x = 0;
		self.data_buf = 0;
		self.in_odd_frame = false;

		self.cycle = 0;
		self.scanline = self.last_scanline;
	}

	pub fn step(&mut self) {
		if (0..=239).contains(&self.scanline) {
			self.render_step();
		} if self.scanline == 241 {
			self.cart.as_mut().mapper.notify_ppu_state(PpuState::Vblank);

			if self.cycle == 1 {
				self.frame_ready = Some(());
				self.stat.set(Stat::vblank, !self.vblank_suppress);

				if self.ctrl.contains(Ctrl::nmi_enabled) && !self.nmi_suppress {
					self.nmi_tmp = Some(());
				}
			}
		} else if self.scanline == self.last_scanline {
			self.render_step();

			if self.cycle == 1 {
				self.stat = Stat::empty();
				self.oam_addr = 0;
			} else if self.cycle == 304 {
				self.reset_render_y();
			} else if self.cart.as_mut().header.timing != ConsoleTiming::PAL 
				&& self.cycle == 339 && self.in_odd_frame
				&& self.rendering_enabled()
			{
				// Odd cycle skip, this isn't present in PAL
				self.cycle += 1;
			}
		}

		// This is needed for Battletoads tigh timings
		if self.mask_write_delay > 0 {
			self.mask_write_delay -= 1;
			if self.mask_write_delay == 0 {
				self.mask = Mask::from_bits_retain(self.mask_tmp);
			}
		}

		self.cycle += 1;
		if self.cycle > 340 {
			self.cycle = 0;
			self.scanline += 1;
			if self.scanline > self.last_scanline {
				self.scanline = 0;
				self.in_odd_frame = !self.in_odd_frame;
				
				self.nmi_suppress = false;
				self.vblank_suppress = false;
			}
		}
	}

	pub(self) fn rendering_enabled(&self) -> bool {
		self.mask.contains(Mask::bg_enabled)
		|| self.mask.contains(Mask::spr_enabled)
	}

	fn map_address(&self, addr: u16) -> (VramDst, usize) {
		match addr {
			0x0000..=0x1FFF => (VramDst::Patterntbl, addr as usize),
			0x2000..=0x2FFF => {
				// let mirrored = self.mirror_nametbl(addr);
				(VramDst::Nametbl, addr as usize)
			}
			0x3F00..=0x3FFF => {
				let palette = self.mirror_palette(addr);
				(VramDst::Palettes, palette as usize)
			}
			_ => (VramDst::Unused, 0),
		}
	}

	pub fn peek_vram(&self, addr: u16) -> u8 {
		let (dst, addr) = self.map_address(addr);
		match dst {
			VramDst::Patterntbl | VramDst::Nametbl => self.cart.as_mut()
				.vram_read(addr),
			VramDst::Palettes => self.palettes[addr],
			VramDst::Unused => 0,
		}
	}

	fn increase_vram_address(&mut self) {
		// https://www.nesdev.org/wiki/PPU_scrolling#$2007_(PPUDATA)_reads_and_writes
		if (0..=239).contains(&self.scanline) || self.scanline == self.last_scanline {
			self.increase_coarse_x();
			self.increase_coarse_y();
		}
		
		self.v.0 = self.v.0.wrapping_add(self.ctrl.vram_addr_incr());
	}

	pub fn read_vram(&mut self) -> u8 {
		// palettes shouldn't be buffered
		let res = if self.v.0 >= PALETTES {
			self.peek_vram(self.v.0)
		} else {
			self.data_buf
		};

		self.data_buf = self.peek_vram(self.v.0);
		self.increase_vram_address();
		
		res
	}

	pub fn write_vram(&mut self, val: u8) {
		let (dst, addr) = self.map_address(self.v.0);
		match dst {
			VramDst::Patterntbl | VramDst::Nametbl => self.cart.as_mut()
				.vram_write(addr, val),
			VramDst::Palettes => self.palettes[addr] = val & 0b0011_1111,
			VramDst::Unused => {}
		}

		self.increase_vram_address();
	}

	pub fn read_reg(&mut self, addr: u16) -> u8 {
		match addr {
			0x2002 => {
				if self.scanline == 241 && (0..3).contains(&self.cycle) {
					if self.cycle == 0 {
						self.vblank_suppress = true;
						self.stat.remove(Stat::vblank);
					} else if self.cycle == 1 || self.cycle == 2 {
						self.stat.insert(Stat::vblank);
					}

					self.nmi_suppress = true;
					self.nmi_requested = None;
					self.nmi_tmp = None;
				}

				let old_stat = self.stat.bits();
				self.w = WriteLatch::FirstWrite;
				self.stat.remove(Stat::vblank);
				old_stat
			}
			0x2004 => self.oam[self.oam_addr as usize],
			0x2007 => self.read_vram(),
			_ => 0,
		}
	}

	pub fn write_reg(&mut self, addr: u16, val: u8) {
		match addr {
			0x2000 => {
				// TODO: bit 0 race condition

				let was_nmi_off = !self.ctrl.contains(Ctrl::nmi_enabled);
				self.ctrl = Ctrl::from_bits_retain(val);

				self.t.set_nametbl_x(val & 0b01);
				self.t.set_nametbl_y((val & 0b10) >> 1);

				if was_nmi_off
					&& self.ctrl.contains(Ctrl::nmi_enabled)
					&& self.stat.contains(Stat::vblank)
				{
					self.nmi_tmp = Some(());
				}

				self.cart.as_mut().mapper.notify_ppuctrl(self.ctrl.bits());
			}
			0x2001 => {
				self.mask_tmp = val;
				self.mask_write_delay = 3;
				self.cart.as_mut().mapper.notify_ppumask(val);
			}
			0x2003 => self.oam_addr = val,
			0x2004 => {
				self.oam[self.oam_addr as usize] = val;
				self.oam_addr = self.oam_addr.wrapping_add(1);
			}
			0x2005 => {
				match self.w {
					WriteLatch::FirstWrite => {
						self.t.set_coarse_x((val & 0b1111_1000) >> 3);
						self.x = val & 0b0000_0111;
						self.w = WriteLatch::SecondWrite;
					}
					WriteLatch::SecondWrite => {
						let high = (val & 0b1111_1000) >> 3;
						let low = val & 0b0000_0111;
						self.t.set_coarse_y(high);
						self.t.set_fine_y(low);
						self.w = WriteLatch::FirstWrite;
					}
				}
			}
			0x2006 => {
				match self.w {
					WriteLatch::FirstWrite => {
						// val is set to low byte of t
						self.t.0 = ((val as u16) << 8) | (self.t.0 & 0x00FF);
						// cut bit 14 and 15
						self.t.0 = self.t.0 & 0x3FFF;
						self.w = WriteLatch::SecondWrite;
					}
					WriteLatch::SecondWrite => {
						// val is set to high byte of t
						self.t.0 = (self.t.0 & 0xFF00) | (val as u16);
						self.v.0 = self.t.0;

						self.w = WriteLatch::FirstWrite;
					}
				}
			}
			0x2007 => self.write_vram(val),
			_ => {},
		}
	}

	fn mirror_palette(&self, addr: u16) -> u16 {
		let addr = (addr - PALETTES) % 32;
		if addr >= 16 && addr % 4 == 0 { addr - 16 }
		else { addr }
	}
}