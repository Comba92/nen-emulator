use nen_emulator::ppu::frame::RGBColor;

#[test]
fn load_palette() {
    let palette = include_bytes!("../palettes/Composite_wiki.pal");

    println!("{:?}", palette);
    let colors: Vec<RGBColor> = palette
        .chunks(3)
        .take(32)
        .map(|rgb| RGBColor(rgb[0], rgb[1], rgb[2]))
        .collect();

    let array: [RGBColor; 32] = colors.try_into().unwrap();
    println!("{:?}", array);
}
