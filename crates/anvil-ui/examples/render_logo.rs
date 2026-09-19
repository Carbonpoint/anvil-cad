//! Rasterizes the anvil logo (the window icon pixels) to a PPM so it can
//! be looked at: `cargo run -p anvil-ui --example render_logo -- out.ppm`
//! Convert with `scripts/ppm2png.py out.ppm out.png`.

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "logo.ppm".into()));
    let size = 256u32;
    let rgba = anvil_ui::icons::logo_rgba(size);
    let mut bytes = format!("P6 {size} {size} 255\n").into_bytes();
    for px in rgba.chunks(4) {
        // Flatten onto a light grey background so transparency is visible.
        let a = px[3] as f32 / 255.0;
        let bg = 235.0f32;
        let r = px[0] as f32 * a + bg * (1.0 - a);
        let g = px[1] as f32 * a + bg * (1.0 - a);
        let b = px[2] as f32 * a + bg * (1.0 - a);
        bytes.extend_from_slice(&[r as u8, g as u8, b as u8]);
    }
    std::fs::write(&out, bytes).unwrap();
    println!("wrote {}", out.display());
}
