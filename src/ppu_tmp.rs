
  // pub fn ppu_step(&mut self) {
  //   // TODO: lookup table handlers?
  //   match self.ppu.state {
  //     RenderState::PreRenderScanline => {
  //       // no drawing occurs here
  //       self.fetch_step();

  //       let ppu = &mut self.ppu;
        
  //       if ppu.cycle == 304 {
  //         ppu.v.set_nametbl_y(ppu.t.nametbl_y());
  //         ppu.v.set_coarse_y(ppu.t.coarse_y());
  //         ppu.v.set_fine_y(ppu.t.fine_y());
  //       }

  //       else if (ppu.odd_frame && ppu.cycle == 339) 
  //         || (!ppu.odd_frame && ppu.cycle == 340) 
  //       {
  //         ppu.state = RenderState::FirstIdleCycle;
  //         ppu.scanline = 0;
  //         ppu.cycle = -1;
  //         ppu.odd_frame = !ppu.odd_frame;
  //         self.frame_ready = true;
  //       }
  //     }
  //     RenderState::FirstIdleCycle => self.ppu.state = RenderState::RenderingBg,
  //     RenderState::RenderingBg => {
  //       self.push_pixel();
  //       self.fetch_step();
        
  //       let ppu = &mut self.ppu;
  //       if ppu.cycle == 240 {
  //         // wrap coarse x here
  //         ppu.v.set_coarse_x(0);
  //         ppu.v.set_nametbl_x(ppu.v.nametbl_x() ^ 1);
          
  //         // unused tile fetch
  //         ppu.state = RenderState::RenderingUnusedBg;
  //       }
  //     }
  //     RenderState::RenderingUnusedBg => {
  //       let ppu = &mut self.ppu;
        
  //       if ppu.cycle == 256 {
  //         // y increment
  //         let v = &mut ppu.v;
  //         if v.fine_y() < 7 {        // if fine Y < 7
  //           v.set_fine_y(v.fine_y() + 1);          // increment fine Y
  //         } else {                  
  //           v.set_fine_y(0);                    // fine Y = 0
  //           let mut y = v.coarse_y();        // let y = coarse Y
  //           if y == 29 {
  //             y = 0;                         // coarse Y = 0
  //             v.set_nametbl_y(v.nametbl_y() ^ 1);    // switch vertical nametable
  //           } else if y == 31 {
  //             y = 0;                        // coarse Y = 0, nametable not switched
  //           } else {
  //             y += 1;
  //           }                         // increment coarse Y
  //           v.set_coarse_y(y);     // put coarse Y back into v
  //         }

  //         ppu.v.set_coarse_x(ppu.t.coarse_x());
  //         ppu.v.set_nametbl_x(ppu.t.nametbl_x());

  //         // TODO: sprite evaluation for next scanline
  //         ppu.state = RenderState::RenderingSpr;
  //       }
  //     }
  //     RenderState::RenderingSpr => {
  //       // TODO: sprites fetchin and drawing

  //       if self.ppu.cycle == 320 {
  //         self.ppu.state = RenderState::RenderingEnd;
  //       }
  //     }
  //     RenderState::RenderingEnd => {
  //       // no drawing here, only fetches to the first two tiles for next scanline
  //       self.fetch_step();

  //       let ppu= &mut self.ppu;
  //       if ppu.cycle == 340 {
  //         if ppu.scanline == 239 {
  //           ppu.state = RenderState::PostRenderScanline;
  //           ppu.pixels_count = 0;
  //         } else {
  //           ppu.state = RenderState::FirstIdleCycle;
  //         }
  //         ppu.cycle = -1;
  //         ppu.scanline += 1;
  //       }
  //     }
  //     RenderState::PostRenderScanline => {
  //       // do nothing

  //       let ppu = &mut self.ppu;
  //       if ppu.scanline == 241 && ppu.cycle == 1 {
  //         ppu.state = RenderState::Vblank;
          
  //         ppu.stat.insert(Status::Vblank);
  //         self.interrupts.insert(emu::Interrupts::NMI);
  //       }
  //     }
  //     RenderState::Vblank => {
  //       // do nothing

  //       let ppu = &mut self.ppu;
  //       if ppu.scanline == 261 && ppu.cycle == 1 {
  //         ppu.state = RenderState::PreRenderScanline;
  //         ppu.stat.clear();
  //         // first pre render line fetch
  //         self.fetch_step();
  //       }
  //     }
  //   }

  //   if self.ppu.cycle == 340 {
  //     self.ppu.scanline += 1;
  //     self.ppu.cycle = -1;
  //   }
  //   self.ppu.cycle += 1;
  // }

  // pub fn ppu_step(&mut self)  {
  //   let kind = &SCANLINE_KINDS.get(&self.ppu.scanline);

  //   match kind {
  //     Some(ScanlineKind::Vblank) => {}
  //     Some(ScanlineKind::PostRender) => {
  //       if self.ppu.cycle == 1 {
  //         self.ppu.stat.insert(Status::Vblank);
  //         if self.ppu.ctrl.vblank_nmi_enabled {
  //           self.interrupts.insert(emu::Interrupts::NMI);
  //         }
  //       }
  //     }
  //     Some(ScanlineKind::PreRender) => {
  //       if self.ppu.cycle == 1 {
  //         self.ppu.stat.clear();
  //       } else if self.ppu.cycle == 280 || self.ppu.cycle == 304 {
  //         self.ppu.scroll_y_tx();
  //       }

  //       let cycle = &CYCLE_KINDS.get(&self.ppu.cycle);
  //       match cycle {
  //         Some(CycleKind::Idle) => {}
  //         Some(CycleKind::ScrollYInc) => self.ppu.scroll_y_inc(),
  //         Some(CycleKind::ScrollXTx) => {
  //           self.ppu.scroll_x_tx();
  //           // TODO: first spr step
  //         }
  //         Some(CycleKind::SprRender) => {
  //           // TODO: spr step
  //         }
  //         Some(CycleKind::BgPreFetch) | None => self.fetch_step(),
  //       }
  //     }
  //     None => {
  //       let cycle = &CYCLE_KINDS.get(&self.ppu.cycle);
  //       match cycle {
  //         Some(CycleKind::Idle) => {}
  //         Some(CycleKind::ScrollYInc) => self.ppu.scroll_y_inc(),
  //         Some(CycleKind::ScrollXTx) => {
  //           self.ppu.scroll_x_tx();
  //           // TODO: first spr step
  //         }
  //         Some(CycleKind::SprRender) => {
  //           // TODO: spr step
  //         }
  //         Some(CycleKind::BgPreFetch) => self.fetch_step(),
  //         None => {
  //           self.fetch_step();
  //           self.push_pixel();
  //         }
  //       }
  //     }
  //   }

  //   let ppu = &mut self.ppu;

  //   ppu.cycle += 1;
  //   if ppu.cycle > 340 {
  //     ppu.cycle = 0;
  //     ppu.scanline += 1;

  //     // TODO: odd frame skip
  //     if ppu.scanline > 261 {
  //       ppu.scanline = 0;
  //       self.frame_ready = true; 
  //     }
  //   }
  // }


// enum ScanlineKind {
//   Vblank,
//   PostRender,
//   PreRender,
// }

// static SCANLINE_KINDS: LazyLock<HashMap<i16, ScanlineKind>> = LazyLock::new(|| {
//   let mut map = HashMap::with_capacity(23);
    
//   map.extend([
//     (240, ScanlineKind::Vblank),
//     (241, ScanlineKind::PostRender),
//     (261, ScanlineKind::PreRender),
//   ]);

//   for line in 241..261 {
//     map.insert(line, ScanlineKind::Vblank);
//   }

//   map
// });

// enum CycleKind {
//   Idle,
//   ScrollYInc,
//   ScrollXTx,
//   SprRender,
//   BgPreFetch,
// }

// static CYCLE_KINDS: LazyLock<HashMap<i16, CycleKind>> = LazyLock::new(|| {
//   let mut map = HashMap::with_capacity(87);

//   map.extend([
//     (0, CycleKind::Idle),
//     (256, CycleKind::ScrollYInc),
//     (257, CycleKind::ScrollXTx),
//     (337, CycleKind::Idle),
//     (338, CycleKind::Idle),
//     (339, CycleKind::Idle),
//     (340, CycleKind::Idle),
//   ]);

//   for cycle in 258..321 {
//     map.insert(cycle, CycleKind::SprRender);
//   }

//   for cycle in 321..337 {
//     map.insert(cycle, CycleKind::BgPreFetch);
//   }

//   map
// });