//! Offscreen render of the business card sample:
//! `cargo run -p anvil-ui --example render_card -- out.ppm`
fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "card.ppm".into());
    let doc = anvil_io::business_card("Alex Goldman", "https://www.linkedin.com/");
    let (w, h) = (1000usize, 700usize);
    let img = anvil_ui::render_offscreen(&doc, w, h);
    let mut bytes = format!("P6 {w} {h} 255\n").into_bytes();
    for px in &img.pixels {
        bytes.extend_from_slice(&[px.r(), px.g(), px.b()]);
    }
    std::fs::write(&out, bytes).unwrap();
}
