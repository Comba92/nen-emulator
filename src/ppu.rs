use crate::{cart::Mirroring, frame::NesScreen, mapper::CartMapper};
use bitfield_struct::bitfield;
use bitflags::bitflags;
use render::Renderer;

mod render;

bitflags! {
	#[derive(Debug)]
	struct Ctrl: u8 {
		const base_nametbl  = 0b0000_0011;
		const vram_incr     = 0b0000_0100;
		const spr_ptrntbl   = 0b0000_1000;

		const bg_ptrntbl    = 0b0001_0000;
		const spr_size      = 0b0010_0000;
		const master_slave  = 0b0100_0000;
		const vblank_nmi_on = 0b1000_0000;
	}

	#[derive(Debug)]
	struct Mask: u8 {
		const greyscale     = 0b0000_0001;
		const bg_lstrip     = 0b0000_0010;
		const spr_lstrip    = 0b0000_0100;
		const bg_render_on  = 0b0000_1000;

		const spr_render_on = 0b0001_0000;
		const red_boost     = 0b0010_0000;
		const blue_boost    = 0b0100_0000;
		const green_boost   = 0b1000_0000;
	}

	#[derive(Debug)]
	struct Stat: u8 {
		const open_bus     = 0b0001_1111;
		const spr_overflow = 0b0010_0000;
		const spr0_hit     = 0b0100_0000;
		const vblank       = 0b1000_0000;
	}
}

impl Ctrl {
	pub fn base_nametbl_addr(&self) -> u16 {
		let nametbl_idx = self.bits() & Ctrl::base_nametbl.bits();
		0x2000 + 0x0400 * nametbl_idx as u16
	}

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
		match self.contains(Ctrl::spr_size) {
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

#[derive(Debug)]
enum WriteLatch {
	FirstWrite,
	SecondWrite,
}

enum VramDst {
	Patterntbl,
	Nametbl,
	Palettes,
	Unused,
}

pub const NAMETABLES: u16 = 0x2000;
pub const ATTRIBUTES: u16 = 0x23C0;
pub const PALETTES: u16 = 0x3F00;

pub struct Ppu {
	pub screen: NesScreen,
	renderer: Renderer,

	v: LoopyReg,   // current vram address
	t: LoopyReg,   // temporary vram address / topleft onscreen tile
	x: u8,         // Fine X Scroll
	w: WriteLatch, // First or second write toggle
	data_buf: u8,

	ctrl: Ctrl,
	nmi_skip: bool,
	mask: Mask,
	stat: Stat,
	oam_addr: u8,

	mapper: CartMapper,
	chr: Vec<u8>,
	vram: [u8; 0x800],
	palettes: [u8; 32],
	oam: [u8; 256],

	pub scanline: usize,
	pub cycle: usize,
	in_odd_frame: bool,

	mirroring: Mirroring,

	pub nmi_requested: Option<()>,
	pub vblank_started: Option<()>,
}

impl Ppu {
	pub fn new(chr_rom: Vec<u8>, mapper: CartMapper, mirroring: Mirroring) -> Self {
		let mapper_mirroring = mapper.borrow().mirroring();

		Self {
			screen: NesScreen::default(),
			renderer: Renderer::new(),

			v: LoopyReg::new(),
			t: LoopyReg::new(),
			x: 0,
			w: WriteLatch::FirstWrite,

			chr: chr_rom,
			mapper,
			vram: [0; 0x800],
			palettes: [0; 32],
			oam: [0; 256],

			oam_addr: 0,
			data_buf: 0,
			in_odd_frame: true,
			scanline: 261,
			cycle: 0,
			ctrl: Ctrl::empty(),
			nmi_skip: false,
			mask: Mask::empty(),
			stat: Stat::empty(),

			mirroring: if let Some(mapper_mirroring) = mapper_mirroring {
				mapper_mirroring
			} else {
				mirroring
			},

			nmi_requested: None,
			vblank_started: None,
		}
	}

	pub fn reset(&mut self) {
		// TODO: better ppu resetting, this works for now
		*self = Ppu::new(self.chr.clone(), self.mapper.clone(), self.mirroring);
	}

	pub fn step(&mut self) {
		match self.scanline {
			(0..=239) => self.render_step(),
			241 => {
				if self.cycle == 1 {
					self.stat.insert(Stat::vblank);
					self.vblank_started = Some(());

					if self.ctrl.contains(Ctrl::vblank_nmi_on)
						&& !self.nmi_skip
					{
						self.nmi_requested = Some(());
					}
				}
			}
			261 => {
				if self.cycle == 1 {
					self.stat = Stat::empty();
					self.nmi_skip = false;
					self.oam_addr = 0;
				} else if (280..=304).contains(&self.cycle) {
					self.reset_render_y();
				} else if (321..=336).contains(&self.cycle) {
					self.bg_step();
				} else if self.cycle >= 339 && self.in_odd_frame
				&& self.rendering_enabled() {
					// Odd cycle skip
					self.cycle += 1;
				}
			}
			_ => {}
		}

		self.cycle += 1;
		if self.cycle >= 341 {
			self.cycle = 0;
			self.scanline += 1;
			if self.scanline >= 262 {
				self.scanline = 0;
				self.in_odd_frame = !self.in_odd_frame;
			}
		}
	}

	pub(self) fn rendering_enabled(&self) -> bool {
		self.mask.contains(Mask::bg_render_on)
		|| self.mask.contains(Mask::spr_render_on)
	}

	fn map_address(&self, addr: u16) -> (VramDst, usize) {
		match addr {
			0x0000..=0x1FFF => (VramDst::Patterntbl, addr as usize),
			0x2000..=0x2FFF => {
				let mirrored = self.mirror_nametbl(addr);
				(VramDst::Nametbl, mirrored as usize)
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
			VramDst::Patterntbl => self.mapper.borrow_mut().read_chr(&self.chr, addr),
			VramDst::Nametbl => self.vram[addr],
			VramDst::Palettes => self.palettes[addr],
			VramDst::Unused => 0,
		}
	}

	fn increase_vram_address(&mut self) {
		self.v.0 = self.v.0.wrapping_add(self.ctrl.vram_addr_incr());
	}

	pub fn read_vram(&mut self) -> u8 {
		// palettes shouldn't be buffered
		let res = if self.v.0 >= 0x3F00 {
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
			VramDst::Patterntbl => self.mapper.borrow_mut().write_chr(&mut self.chr, addr, val),
			VramDst::Nametbl => self.vram[addr] = val,
			VramDst::Palettes => self.palettes[addr] = val & 0b0011_1111,
			VramDst::Unused => {}
		}

		self.increase_vram_address();
	}

	pub fn read_reg(&mut self, addr: u16) -> u8 {
		match addr {
			0x2002 => {
				if self.scanline == 241 && (0..3).contains(&self.cycle) {
					self.nmi_skip = true;
					self.nmi_requested = None;
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

				let was_nmi_off = !self.ctrl.contains(Ctrl::vblank_nmi_on);
				self.ctrl = Ctrl::from_bits_retain(val);
				self.t.set_nametbl_x(val & 0b01);
				self.t.set_nametbl_y((val & 0b10) >> 1);

				if was_nmi_off
					&& self.ctrl.contains(Ctrl::vblank_nmi_on)
					&& self.stat.contains(Stat::vblank)
				{
					self.nmi_requested = Some(());
				}
			}
			0x2001 => self.mask = Mask::from_bits_retain(val),
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

	
	// Horizontal:
	// 0x0800 [ B ]  [ A ] [ a ]
	// 0x0400 [ A ]  [ B ] [ b ]

	// Vertical:
	// 0x0800 [ B ]  [ A ] [ B ]
	// 0x0400 [ A ]  [ a ] [ b ]

	// Single-page: (based on mapper register)
	// 0x0800 [ B ]  [ A ] [ a ]    [ B ] [ b ]
	// 0x0400 [ A ]  [ a ] [ a ] or [ b ] [ b ]
	fn mirror_nametbl(&self, addr: u16) -> u16 {
		let addr = addr - 0x2000;
		let nametbl_idx = addr / 0x400;

		use Mirroring::*;
		// TODO: consider moving this only on the mapper
		let mirroring = if let Some(mirroring) = self.mapper.borrow().mirroring() {
			mirroring
		} else {
			self.mirroring
		};

		match (mirroring, nametbl_idx) {
			(Horizontally, 1) | (Horizontally, 2) => addr - 0x400,
			(Horizontally, 3) => addr - 0x400 * 2,
			(Vertically, 2) | (Vertically, 3) => addr - 0x400 * 2,
			(SingleScreenFirstPage, _) => addr % 0x400,
			(SingleScreenSecondPage, _) => (addr % 0x400) + 0x400,
			(FourScreen, _) => todo!("Four screen mirroring not implemented"),
			_ => addr,
		}
	}

	fn mirror_palette(&self, addr: u16) -> u16 {
		let addr = addr - 0x3F00;
		if addr >= 16 && addr % 4 == 0 {
			addr - 16
		} else {
			addr % 32
		}
	}
}