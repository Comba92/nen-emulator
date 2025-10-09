const PRE_SHIFT: usize = 32;
const TIME_BITS: usize = PRE_SHIFT + 20;
const TIME_UNIT: usize = 1 << TIME_BITS;
const BASS_SHIFT: usize = 9;
const END_FRAME_EXTRA: usize = 2;

const HALF_WIDTH: usize = 8;
const BUF_EXTRA: usize = HALF_WIDTH*2 + END_FRAME_EXTRA;
const PHASE_BITS: usize = 5;
const PHASE_COUNT: usize = 1 << PHASE_BITS;
const DELTA_BITS: usize = 15;
const DELTA_UNIT: usize = 1 << DELTA_BITS;
const FRAC_BITS: usize = TIME_BITS - PRE_SHIFT;

const BLIP_MAX_RATIO: usize = 1 << 20;

pub struct BlipBuf {
  factor: usize,
  offset: usize,
  pub avail: usize,
  integrator: isize,
  samples: Vec<isize>,
}

impl BlipBuf {
  pub fn new(size: usize) -> Self {
    Self {
      factor: TIME_UNIT / BLIP_MAX_RATIO,
      offset: TIME_UNIT / BLIP_MAX_RATIO / 2,
      avail: 0,
      integrator: 0,
      samples: vec![0; size + BUF_EXTRA],
    }
  }

  pub fn set_rates(&mut self, clock_rate: f64, sample_rate: f64) -> Result<(), &'static str> {
    let factor = TIME_UNIT as f64 * sample_rate / clock_rate;
    let factor_decimal = factor - (factor as usize) as f64;

    if !matches!(factor_decimal, 0.0 .. 1.0) {
      return Err("clock rate exceeds maximum, relative to sample rate")
    }
    
    self.factor = factor.ceil() as usize;
    Ok(())
  }

  pub fn clear(&mut self) {
    self.offset = self.factor / 2;
    self.avail = 0;
    self.integrator = 0;
    self.samples.fill(0);
  }

  pub fn clocks_needed(&self, samples_count: usize) -> Result<usize, &'static str> {
    if self.avail + samples_count > self.samples.len() { return Err("buffer can't hold that many more samples") }

    let needed = samples_count * TIME_UNIT;
    if needed < self.offset { return Ok(0) }

    let res = (needed - self.offset + self.factor - 1) / self.factor; 
    Ok(res)
  }

  fn remove_samples(&mut self, count: usize) {
    let remain = (self.avail + BUF_EXTRA).saturating_sub(count);
    self.avail = self.avail.saturating_sub(count);

    self.samples.copy_within(count..count+remain, 0);
    self.samples[remain..].fill(0);
  }

  pub fn read_samples(&mut self, out: &mut [i16], stereo: bool) -> usize {
    let count = out.len().min(self.avail);
    if count == 0 { return count }

    let step = if stereo { 2 } else { 1 };
    let mut sum = self.integrator;

    for i in 0..count {
      let s = (sum >> DELTA_BITS).clamp(i16::MIN as isize, i16::MAX as isize);
      out[i * step] = s as i16;

      sum += self.samples[i] as isize;
      sum -= s << (DELTA_BITS - BASS_SHIFT);
    }

    self.integrator = sum;
    self.remove_samples(count);

    count
  }

  pub fn end_frame(&mut self, clock_duration: usize) -> Result<(), &'static str> {
    let off = clock_duration * self.factor + self.offset;
    let avail = self.avail + (off >> TIME_BITS);
    if avail > self.samples.len() { return Err("buffer size was exceeded") }

    self.avail = avail;
    self.offset = off & (TIME_UNIT - 1);
    Ok(())
  }

  pub fn add_delta(&mut self, time: usize, delta: f64) {
    let fixed = (time * self.factor + self.offset) >> PRE_SHIFT;
    let out = self.avail + (fixed >> FRAC_BITS);
    // if out > self.samples.len() { return Err("buffer size was exceeded") }

    const PHASE_SHIFT: usize = FRAC_BITS - PHASE_BITS;
    let phase = fixed >> PHASE_SHIFT & (PHASE_COUNT - 1);
    let phase_rev = PHASE_COUNT - phase;

    let interp = fixed >> (PHASE_SHIFT - DELTA_BITS) & (DELTA_UNIT - 1);
    let delta2 = (delta as isize * interp as isize) >> DELTA_BITS as isize;
    let delta1 = delta as isize - delta2;

    for i in 0..8 {
      self.samples[out+i]   += BL_STEP[phase][i]*delta1 + BL_STEP[phase+1][i]*delta2;
    }
    for i in 0..8 {
      self.samples[out+8+i] += BL_STEP[phase_rev][7-i]*delta1 + BL_STEP[phase_rev-1][7-i]*delta2;
    }
  }

  pub fn add_delta_fast(&mut self, time: usize, delta: f64) {
    let fixed = (time * self.factor + self.offset) >> PRE_SHIFT;
    let out = self.avail + (fixed >> FRAC_BITS);
    // if out > self.samples.len() { return Err("buffer size was exceeded") }

    let interp = fixed >> (FRAC_BITS - DELTA_BITS) & (DELTA_UNIT - 1);
    let delta2 = delta as isize * interp as isize;
    let delta1 = delta as isize * DELTA_UNIT as isize;

    self.samples[out + 7] += delta1 - delta2;
    self.samples[out + 8] += delta2;
  }
}

const BL_STEP: &[[isize; 8]] = &[
  [43, -115, 350, -488, 1136, -914, 5861, 21022],
  [44, -118, 348, -473, 1076, -799, 5274, 21001],
  [45, -121, 344, -454, 1011, -677, 4706, 20936],
  [46, -122, 336, -431, 942, -549, 4156, 20829],
  [47, -123, 327, -404, 868, -418, 3629, 20679],
  [47, -122, 316, -375, 792, -285, 3124, 20488],
  [47, -120, 303, -344, 714, -151, 2644, 20256],
  [46, -117, 289, -310, 634, -17, 2188, 19985],
  [46, -114, 273, -275, 553, 117, 1758, 19675],
  [44, -108, 255, -237, 471, 247, 1356, 19327],
  [43, -103, 237, -199, 390, 373, 981, 18944],
  [42, -98, 218, -160, 310, 495, 633, 18527],
  [40, -91, 198, -121, 231, 611, 314, 18078],
  [38, -84, 178, -81, 153, 722, 22, 17599],
  [36, -76, 157, -43, 80, 824, -241, 17092],
  [34, -68, 135, -3, 8, 919, -476, 16558],
  [32, -61, 115, 34, -60, 1006, -683, 16001],
  [29, -52, 94, 70, -123, 1083, -862, 15422],
  [27, -44, 73, 106, -184, 1152, -1015, 14824],
  [25, -36, 53, 139, -239, 1211, -1142, 14210],
  [22, -27, 34, 170, -290, 1261, -1244, 13582],
  [20, -20, 16, 199, -335, 1301, -1322, 12942],
  [18, -12, -3, 226, -375, 1331, -1376, 12293],
  [15, -4, -19, 250, -410, 1351, -1408, 11638],
  [13, 3, -35, 272, -439, 1361, -1419, 10979],
  [11, 9, -49, 292, -464, 1362, -1410, 10319],
  [9, 16, -63, 309, -483, 1354, -1383, 9660],
  [7, 22, -75, 322, -496, 1337, -1339, 9005],
  [6, 26, -85, 333, -504, 1312, -1280, 8355],
  [4, 31, -94, 341, -507, 1278, -1205, 7713],
  [3, 35, -102, 347, -506, 1238, -1119, 7082],
  [1, 40, -110, 350, -499, 1190, -1021, 6464],
  [0, 43, -115, 350, -488, 1136, -914, 5861],
];