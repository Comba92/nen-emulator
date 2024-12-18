use std::collections::VecDeque;
use super::{Mask, Ppu, Stat, ATTRIBUTES, NAMETABLES, PALETTES};

pub(super) struct Renderer {
  state: RenderState,
	data: RenderData,
  bg_fifo: VecDeque<(u8, u8)>,
  oam_tmp: VecDeque<OamEntry>,
  spr_scanline: [Option<SprData>; 256]
}
impl Renderer {
  pub fn new() -> Self {
    Self {
      state: RenderState::default(),
      data: RenderData::default(),
      bg_fifo: VecDeque::from([(0,0)].repeat(16)),
      oam_tmp: VecDeque::new(),
      spr_scanline: [const { None } ; 256],
    }
  }
}

#[derive(Default)]
enum RenderState {
  #[default] Nametbl, Attribute, PtrnLow, PtrnHigh
}

#[derive(Default)]
pub (super) struct RenderData {
	pub tile_id: u8,
	pub palette_id: u8,
  pub tile_addr: u16,
	pub tile_plane0: u8,
	pub tile_plane1: u8,
}

#[derive(Default, Clone)]
pub struct SprData {
	pub pixel: u8,
	pub palette_id: u8,
	pub priority: SpritePriority,
	pub is_sprite0: bool,
}

#[derive(Debug, PartialEq, Eq, Default, Clone, Copy)]
pub enum SpritePriority {
    Front,
    #[default]
    Behind,
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct OamEntry {
    pub index: usize,
    pub y: usize,
    pub tile_id: u8,
    pub palette_id: u8,
    pub priority: SpritePriority,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
    pub x: usize,
}
impl OamEntry {
    pub fn from_bytes(bytes: &[u8], index: usize) -> Self {
        let y = bytes[0] as usize;
        let tile = bytes[1];
        let attributes = bytes[2];
        let palette = 4 + (attributes & 0b11);
        let priority = match (attributes >> 5) & 1 != 0 {
            false => SpritePriority::Front,
            true => SpritePriority::Behind,
        };
        let flip_horizontal = attributes >> 6 & 1 != 0;
        let flip_vertical = attributes >> 7 & 1 != 0;

        let x = bytes[3] as usize;

        Self {
            index,
            y,
            tile_id: tile,
            palette_id: palette,
            priority,
            flip_horizontal,
            flip_vertical,
            x,
        }
    }
}

fn pixel_from_planes(bit: u8, plane0: u8, plane1: u8) -> u8 {
  let bit0 = (plane0 >> bit) & 1;
  let bit1 = (plane1 >> bit) & 1;
  (bit1 << 1) | bit0
}

impl Ppu {
  pub(super) fn render_step(&mut self) {
    if (1..=256).contains(&self.cycle) 
    || (321..=336).contains(&self.cycle)
    {
      self.bg_step();
    } else if (257..=320).contains(&self.cycle) {
      if self.cycle == 257 {
        self.evaluate_sprites();
        self.fetch_sprites();
        self.increase_coarse_y();
        self.reset_render_x();
      }
      // self.spr_step();
    }
  }

  fn render_pixel(&mut self) {
    if !self.rendering_enabled() {
      self.screen.0.set_pixel(self.cycle-1, self.scanline, self.color_from_palette(0, 0));
      return;
    }

    let (bg_pixel, bg_palette_id) = self.renderer.bg_fifo
      .get(self.x as usize).unwrap().to_owned();

    let sprite = self.renderer.spr_scanline[self.cycle-1]
      .take().unwrap_or_default();

    if !self.stat.contains(Stat::spr0_hit)
      && sprite.pixel != 0 && bg_pixel != 0
    {
      self.stat.insert(Stat::spr0_hit);
    }

    let pixel_color = if self.mask.contains(Mask::spr_render_on) 
      && sprite.priority == SpritePriority::Front
      && sprite.pixel != 0
    {
      self.color_from_palette(sprite.pixel, sprite.palette_id)
    } else if self.mask.contains(Mask::bg_render_on) {
      self.color_from_palette(bg_pixel, bg_palette_id)
    } else {
      self.color_from_palette(0, 0)
    };

    self.screen.0.set_pixel(self.cycle-1, self.scanline, pixel_color);
  }


  pub(super) fn bg_step(&mut self) {
    self.renderer.bg_fifo.pop_front();
    // We render only during the visilbe frames (1 to 256)
    if self.cycle-1 < 256 { self.render_pixel(); }
    
    // We only do a render step in the odd cycles (each step is 2 cycles long)
    if self.cycle % 2 == 0 {
      match self.renderer.state {
        RenderState::Nametbl => {
          // Load bg fifo
          for i in (0..8).rev() {
            let pixel = pixel_from_planes(
              i,
              self.renderer.data.tile_plane0,
              self.renderer.data.tile_plane1,
            );
            self.renderer.bg_fifo.push_back((pixel, self.renderer.data.palette_id));
          }

          let tile_addr = NAMETABLES + self.v.nametbl_idx();
          self.renderer.data.tile_id = self.peek_vram(tile_addr);
          self.renderer.state = RenderState::Attribute;
        }

        RenderState::Attribute => {
          let attribute_addr = ATTRIBUTES
            + ((self.v.nametbl() as u16) << 10)
            + ((self.v.coarse_y() as u16) / 4) * 8
            + ((self.v.coarse_x() as u16) / 4);

          let attribute = self.peek_vram(attribute_addr);
          let palette_id = self.palette_from_attribute(attribute);

          self.renderer.data.palette_id = palette_id;
          self.renderer.state = RenderState::PtrnLow;
        }

        RenderState::PtrnLow => {
  				let tile_addr = self.ctrl.bg_ptrntbl_addr()
            + (self.renderer.data.tile_id as u16) * 16
            + self.v.fine_y() as u16;

          let plane0 = self.peek_vram(tile_addr);
          self.renderer.data.tile_addr = tile_addr;
          self.renderer.data.tile_plane0 = plane0;
          self.renderer.state = RenderState::PtrnHigh;
        }

        RenderState::PtrnHigh => {
          let plane1 = self
            .peek_vram(self.renderer.data.tile_addr + 8);
          self.renderer.data.tile_plane1 = plane1;
          self.renderer.state = RenderState::Nametbl;

          self.increase_coarse_x();
        }
      }
    }
  }

  // TODO: accurate sprite fetching
  fn spr_step(&mut self) {
    match self.renderer.state {
      RenderState::Nametbl => self.renderer.state = RenderState::Attribute,
      RenderState::Attribute => self.renderer.state = RenderState::PtrnLow,
      RenderState::PtrnLow => {
        let sprite = self.renderer.oam_tmp.pop_front().unwrap_or_default();
			  let dist_from_scanline = self.scanline - sprite.y;
        
        let tile_addr = self.ctrl.spr_ptrntbl_addr()
          + sprite.tile_id as u16 * 16
          + dist_from_scanline as u16;

        self.renderer.data.tile_addr = tile_addr;
        self.renderer.data.tile_plane0 =  self.peek_vram(tile_addr);
        self.renderer.state = RenderState::PtrnHigh;
      }
      RenderState::PtrnHigh => {
        let plane1 =  self
          .peek_vram(self.renderer.data.tile_addr + 8);

        self.renderer.data.tile_plane1 = plane1;
        self.renderer.state = RenderState::Nametbl;
      }
    }
  }

  pub fn evaluate_sprites(&mut self) {
		if !self.rendering_enabled() { return; }
    self.renderer.oam_tmp.clear();

		let mut visible_sprites = 0;
		for i in (0..256).step_by(4) {
			let spr_y = self.oam[i] as isize;
			if spr_y >= 30 * 8 { continue; }
			let dist_from_scanline = self.scanline as isize - spr_y;

			if dist_from_scanline >= 0 && dist_from_scanline < self.ctrl.spr_height() as isize {
				if self.renderer.oam_tmp.len() < 8 {
					self.renderer.oam_tmp
						.push_back(OamEntry::from_bytes(&self.oam[i..i + 4], i));
				}
				visible_sprites += 1;
			}
		}

		let spr_overflow = self.stat.contains(Stat::spr_overflow)
			|| (self.rendering_enabled() && visible_sprites > 8);
		self.stat.set(Stat::spr_overflow, spr_overflow);
	}

  pub fn fetch_sprites(&mut self) {
		for sprite in self.renderer.oam_tmp.iter() {
			let vertical_start: usize = if sprite.flip_vertical { 7 } else { 0 };
			let dist_from_scanline = self.scanline - sprite.y;

			let spr_addr = match self.ctrl.spr_height() {
				8 => {
					self.ctrl.spr_ptrntbl_addr()
						+ sprite.tile_id as u16 * 16
						+ (dist_from_scanline).abs_diff(vertical_start) as u16
				}
				16 => {
					let tbl = (sprite.tile_id & 1) as u16;
					let mut tile_id = sprite.tile_id as u16 & 0b1111_1110;
					tile_id += match sprite.flip_vertical {
						false => {
							if dist_from_scanline >= 8 {
								1
							} else {
								0
							}
						}
						true => {
							if dist_from_scanline >= 8 {
								0
							} else {
								1
							}
						}
					};

					(tbl << 12)
						+ tile_id * 16
						+ (dist_from_scanline % 8).abs_diff(vertical_start) as u16
				}
				_ => unreachable!("sprite heights are either 8 or 16"),
			};

			let mut plane0 = self.peek_vram(spr_addr);
			let mut plane1 = self.peek_vram(spr_addr + 8);

			// TODO: eventually fix this hack
			if !sprite.flip_horizontal {
				plane0 = plane0.reverse_bits();
				plane1 = plane1.reverse_bits();
			}

			for i in (0..8usize).rev() {
				if sprite.x + i >= 32 * 8 {
					continue;
				}

				// sprite with higher priority already there
				if let Some(current_pixel) = &self.renderer.spr_scanline[sprite.x + i] {
					if current_pixel.pixel != 0 {
						continue;
					}
				}

				let pixel = pixel_from_planes(i as u8, plane0, plane1);
				self.renderer.spr_scanline[sprite.x + i] = Some(SprData {
					pixel,
					palette_id: sprite.palette_id,
					priority: sprite.priority,
					is_sprite0: sprite.index == 0,
				});
			}
		}
	}
}

impl Ppu {
  // TODO: can do this be better?
	fn palette_from_attribute(&self, attribute: u8) -> u8 {
		match (self.v.coarse_x() % 4, self.v.coarse_y() % 4) {
			(0..2, 0..2) => (attribute & 0b0000_0011) >> 0 & 0b11,
			(2..4, 0..2) => (attribute & 0b0000_1100) >> 2 & 0b11,
			(0..2, 2..4) => (attribute & 0b0011_0000) >> 4 & 0b11,
			(2..4, 2..4) => (attribute & 0b1100_0000) >> 6 & 0b11,
			_ => unreachable!("mod 4 should always give value smaller than 4"),
		}
	}

  fn color_from_palette(&self, pixel: u8, palette_id: u8) -> u8 {
    self.peek_vram(PALETTES + (4*palette_id + pixel) as u16)
	}

  // https://www.nesdev.org/wiki/PPU_scrolling#Wrapping_around
	fn increase_coarse_x(&mut self) {
    if !self.rendering_enabled() { return; }
    
		if self.v.coarse_x() == 31 {
			self.v.set_coarse_x(0);
			self.v.set_nametbl_x(self.v.nametbl_x() ^ 1); // flip horizontal nametbl
		} else {
			self.v.set_coarse_x(self.v.coarse_x() + 1);
		}
	}

	// https://www.nesdev.org/wiki/PPU_scrolling#Wrapping_around
	fn increase_coarse_y(&mut self) {
		if !self.rendering_enabled() { return; }

		if self.v.fine_y() < 7 {
			self.v.set_fine_y(self.v.fine_y() + 1);
		} else {
			self.v.set_fine_y(0);
			let mut y = self.v.coarse_y();
			if y == 29 {
				y = 0;
				self.v.set_nametbl_y(self.v.nametbl_y() ^ 1); // flip vertical nametbl
			} else if y == 31 {
				y = 0;
			} else {
				y += 1;
			}

			self.v.set_coarse_y(y);
		}
	}

	// https://forums.nesdev.org/viewtopic.php?p=5578#p5578
	fn reset_render_x(&mut self) {
		if !self.rendering_enabled() { return; }

		self.v.set_coarse_x(self.t.coarse_x());
		self.v.set_nametbl_x(self.t.nametbl_x());
	}

	// https://forums.nesdev.org/viewtopic.php?p=229928#p229928
	pub(super) fn reset_render_y(&mut self) {
		if !self.rendering_enabled() { return; }

		self.v.set_coarse_y(self.t.coarse_y());
		self.v.set_fine_y(self.t.fine_y());
		self.v.set_nametbl_y(self.t.nametbl_y());
	}
}