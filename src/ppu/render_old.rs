use super::{Ppu, Mask, Stat};

#[derive(Default)]
pub (super) struct BgData {
	pub tile_id: u8,
	pub palette_id: u8,
	pub tile_plane0: u8,
	pub tile_plane1: u8,
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
        let palette = 16 + (attributes & 0b11) * 4;
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


#[derive(Default, Clone)]
pub(super) struct SprData {
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
    Background,
}

enum RenderState {

}

impl Ppu {
	pub fn render_step(&mut self) {
		// visible scanlines
		if (1..=256).contains(&self.cycle) || (321..=336).contains(&self.cycle) {
			self.fetch_bg_step();
		}

		if self.cycle == 64 {
			self.oam_buf.clear();
		} else if self.cycle == 256 {
			self.increase_coarse_y();
			self.evaluate_sprites();
		} else if self.cycle == 257 {
			self.reset_render_x();
			self.scanline_sprites.fill(None);
		} else if self.cycle == 320 && self.scanline != 261 {
			self.fetch_sprites();
		}
	}

	// TODO: Clean this shit up...
	pub fn render_pixel(&mut self) {
		if !self.rendering_enabled() && self.cycle <= 32 * 8 && self.scanline <= 30 * 8 {
			self.screen.0.set_pixel(
				self.cycle - 1,
				self.scanline,
				self.get_color_from_palette(0, 0),
			);
		} else if self.rendering_enabled() && self.cycle <= 32 * 8 && self.scanline <= 30 * 8 {
			let (bg_pixel, bg_palette_id) = self.bg_fifo.get(self.x as usize).unwrap().to_owned();

			let spr_data = self.scanline_sprites[self.cycle - 1]
				.clone()
				.unwrap_or_default();

			if spr_data.is_sprite0
	  && spr_data.pixel != 0 && bg_pixel != 0
	  && self.scanline < 239
	  // Should check for 255, but we're putting pixel on the previous current cycle
	  && self.cycle != 256
	  && !(self.cycle <= 8 && (!self.mask.contains(Mask::spr_lstrip) || !self.mask.contains(Mask::bg_lstrip)))
	  && self.mask.contains(Mask::bg_render_on)
	  && self.mask.contains(Mask::spr_render_on)
			{
				self.stat.insert(Stat::spr0_hit);
			}

			if self.mask.contains(Mask::spr_render_on)
				&& !(self.cycle <= 8 && !self.mask.contains(Mask::spr_lstrip))
				&& (spr_data.priority == SpritePriority::Front || bg_pixel == 0)
				&& spr_data.pixel != 0
			{
				let color = self.get_color_from_palette(spr_data.pixel, spr_data.palette_id);
				self.screen
					.0
					.set_pixel(self.cycle - 1, self.scanline, color);
			} else if self.mask.contains(Mask::bg_render_on)
				&& !(self.cycle <= 8 && !self.mask.contains(Mask::bg_lstrip))
			{
				let color = self.get_color_from_palette(bg_pixel, bg_palette_id);
				self.screen
					.0
					.set_pixel(self.cycle - 1, self.scanline, color);
			}
		}
	}

	pub fn evaluate_sprites(&mut self) {
		if !self.rendering_enabled() {
			return;
		}

		let mut visible_sprites = 0;

		for i in (0..256).step_by(4) {
			let spr_y = self.oam[i] as isize;
			if spr_y >= 30 * 8 {
				continue;
			}
			let dist_from_scanline = self.scanline as isize - spr_y;

			if dist_from_scanline >= 0 && dist_from_scanline < self.ctrl.spr_height() as isize {
				if self.oam_buf.len() < 8 {
					self.oam_buf
						.push(OamEntry::from_bytes(&self.oam[i..i + 4], i));
				}
				visible_sprites += 1;
			}
		}

		let spr_overflow = self.stat.contains(Stat::spr_overflow)
			|| (self.rendering_enabled() && visible_sprites > 8);
		self.stat.set(Stat::spr_overflow, spr_overflow);
	}

	pub fn fetch_sprites(&mut self) {
		for sprite in self.oam_buf.iter() {
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
				if let Some(current_pixel) = &self.scanline_sprites[sprite.x + i] {
					if current_pixel.pixel != 0 {
						continue;
					}
				}

				let pixel = self.get_pixel_from_planes(i as u8, plane0, plane1);
				self.scanline_sprites[sprite.x + i] = Some(SprData {
					pixel,
					palette_id: sprite.palette_id,
					priority: sprite.priority,
					is_sprite0: sprite.index == 0,
				});
			}
		}
	}

	pub fn fetch_bg_step(&mut self) {
		self.bg_fifo.pop_front();

		let step = ((self.cycle - 1) % 8) + 1;
		// https://www.nesdev.org/wiki/PPU_scrolling#Tile_and_attribute_fetching
		match step {
			2 => {
				// Load bg fifo
				for i in (0..8).rev() {
					let pixel = self.get_pixel_from_planes(
						i,
						self.bg_buf.tile_plane0,
						self.bg_buf.tile_plane1,
					);
					self.bg_fifo.push_back((pixel, self.bg_buf.palette_id));
				}

				let tile_addr = 0x2000 + self.v.nametbl_idx();
				self.bg_buf.tile_id = self.peek_vram(tile_addr);
			}
			4 => {
				let attribute_addr = 0x23C0
					+ ((self.v.nametbl() as u16) << 10)
					+ ((self.v.coarse_y() as u16) / 4) * 8
					+ ((self.v.coarse_x() as u16) / 4);

				let attribute = self.peek_vram(attribute_addr);
				let palette_id = self.get_palette_from_attribute(attribute);

				self.bg_buf.palette_id = palette_id;
			}
			6 => {
				let tile_addr = self.ctrl.bg_ptrntbl_addr()
					+ (self.bg_buf.tile_id as u16) * 16
					+ self.v.fine_y() as u16;

				let plane0 = self.peek_vram(tile_addr);
				self.bg_buf.tile_plane0 = plane0;
			}
			7 => {
				let tile_addr = self.ctrl.bg_ptrntbl_addr()
					+ (self.bg_buf.tile_id as u16) * 16
					+ self.v.fine_y() as u16;

				let plane1 = self.peek_vram(tile_addr + 8);
				self.bg_buf.tile_plane1 = plane1;
			}
			8 => self.increase_coarse_x(),
			_ => {}
		}

		self.render_pixel();
	}

	fn get_pixel_from_planes(&self, bit: u8, plane0: u8, plane1: u8) -> u8 {
		let bit0 = (plane0 >> bit) & 1;
		let bit1 = (plane1 >> bit) & 1;
		(bit1 << 1) | bit0
	}

	fn get_color_from_palette(&self, pixel: u8, palette_id: u8) -> u8 {
		if pixel == 0 {
			self.peek_vram(0x3F00)
		} else {
			self.peek_vram(0x3F00 + (palette_id + pixel) as u16)
		}
	}

	fn get_palette_from_attribute(&self, attribute: u8) -> u8 {
		4 * match (self.v.coarse_x() % 4, self.v.coarse_y() % 4) {
			(0..2, 0..2) => (attribute & 0b0000_0011) >> 0 & 0b11,
			(2..4, 0..2) => (attribute & 0b0000_1100) >> 2 & 0b11,
			(0..2, 2..4) => (attribute & 0b0011_0000) >> 4 & 0b11,
			(2..4, 2..4) => (attribute & 0b1100_0000) >> 6 & 0b11,
			_ => unreachable!("mod 4 should always give value smaller than 4"),
		}
	}

	// https://www.nesdev.org/wiki/PPU_scrolling#Wrapping_around
	fn increase_coarse_x(&mut self) {
		if !self.rendering_enabled() {
			return;
		}

		if self.v.coarse_x() == 31 {
			self.v.set_coarse_x(0);
			self.v.set_nametbl_x(self.v.nametbl_x() ^ 1); // flip horizontal nametbl
		} else {
			self.v.set_coarse_x(self.v.coarse_x() + 1);
		}
	}

	// https://www.nesdev.org/wiki/PPU_scrolling#Wrapping_around
	fn increase_coarse_y(&mut self) {
		if !self.rendering_enabled() {
			return;
		}

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
		if !self.rendering_enabled() {
			return;
		}

		self.v.set_coarse_x(self.t.coarse_x());
		self.v.set_nametbl_x(self.t.nametbl_x());
	}

	// https://forums.nesdev.org/viewtopic.php?p=229928#p229928
	pub(super) fn reset_render_y(&mut self) {
		if !self.rendering_enabled() {
			return;
		}

		self.v.set_coarse_y(self.t.coarse_y());
		self.v.set_fine_y(self.t.fine_y());
		self.v.set_nametbl_y(self.t.nametbl_y());
	}
}