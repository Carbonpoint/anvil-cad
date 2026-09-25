//! Take a picture of the whole Anvil window without a display.
//!
//! Developer builds only. Examples:
//!
//! ```text
//! cargo run -p anvil-ui --features devtools --example shot -- --list
//! cargo run -p anvil-ui --features devtools --example shot -- \
//!     --sample wb2 --size 1366x768 --text 22 --theme light --out /tmp/wb2.ppm
//! ```
//!
//! The file is a binary PPM. `python3 scripts/ppm2png.py in.ppm out.png`
//! turns it into the PNG that the documents use.

use anvil_ui::shot::{sample_image, unpainted_pixels, write_ppm, ShotOptions, SAMPLES};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return;
    }
    if args.iter().any(|a| a == "--list") {
        for (name, what) in SAMPLES {
            println!("{name:8} {what}");
        }
        return;
    }
    let value =
        |key: &str| -> Option<String> { args.iter().position(|a| a == key).and_then(|i| args.get(i + 1)).cloned() };
    let sample = value("--sample").unwrap_or_else(|| "demo".into());
    let out = value("--out").unwrap_or_else(|| format!("{sample}.ppm"));
    let mut opts = ShotOptions::default();
    if let Some(size) = value("--size") {
        let (w, h) = size.split_once('x').unwrap_or_else(|| panic!("--size wants WxH, got {size}"));
        opts.width = w.parse().expect("width");
        opts.height = h.parse().expect("height");
    }
    if let Some(t) = value("--text") {
        opts.settings.base_text = t.parse().expect("text size");
    }
    if let Some(s) = value("--scale") {
        opts.settings.scale = s.parse().expect("ui scale");
    }
    if let Some(i) = value("--icons") {
        opts.settings.icon_scale = i.parse().expect("icon scale");
    }
    if let Some(n) = value("--select") {
        opts.select = Some(n.parse().expect("feature number"));
    }
    if let Some(t) = value("--tab") {
        opts.tab = Some(t.to_string());
    }
    if args.iter().any(|a| a == "--settings") {
        opts.open_settings = true;
    }
    if let Some(c) = value("--crop") {
        let n: Vec<u32> = c.split(',').map(|v| v.parse().expect("crop wants x,y,w,h")).collect();
        assert!(n.len() == 4, "--crop wants x,y,w,h, got {c}");
        opts.crop = Some((n[0], n[1], n[2], n[3]));
    }
    if let Some(t) = value("--theme") {
        opts.settings.theme = match t.as_str() {
            "light" => egui::ThemePreference::Light,
            "dark" => egui::ThemePreference::Dark,
            _ => egui::ThemePreference::System,
        };
    }
    let t0 = std::time::Instant::now();
    let Some(img) = sample_image(&sample, &opts) else {
        eprintln!("unknown sample {sample}. Use --list to see the names.");
        std::process::exit(2);
    };
    let holes = unpainted_pixels(&img);
    if !holes.is_empty() {
        eprintln!("warning: {} pixels were never painted, first at {:?}", holes.len(), holes.first());
    }
    write_ppm(std::path::Path::new(&out), &img).expect("write the picture");
    eprintln!("{} x {} written to {out} in {:.1} s", opts.width, opts.height, t0.elapsed().as_secs_f64());
}

const USAGE: &str = "\
shot: a picture of the Anvil window, with no display.

  --list             names of the samples
  --sample NAME      which sample to load (default: demo)
  --size WxH         window size in points (default: 1500x950)
  --text N           base text size, 10 to 22 (default: 14)
  --scale N          UI scale, 0.7 to 2.0 (default: 1.0)
  --icons N          icon scale, 0.8 to 2.0 (default: 1.0)
  --theme light|dark|system
  --select N         select feature N in the Part Navigator
  --settings         open the Settings window
  --tab NAME         show this ribbon tab (File, Solid, Field, ...)
  --crop x,y,w,h     keep only this part of the picture
  --out PATH         where to write the PPM (default: NAME.ppm)";
