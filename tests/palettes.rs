use sdl2::pixels::Color;

#[test]
fn load_palette() {
  let palette = include_bytes!("../palettes/Composite_wiki.pal");

  println!("{:?}", palette);
  let colors: Vec<Color> = palette
    .chunks(3)
    .take(32)
    .map(|rgb| Color::RGB(rgb[0], rgb[1], rgb[2]))
    .collect();

  let array: [Color; 32] = colors.try_into().unwrap();
  println!("{:?}", array);
}