//! anvil-cli: headless Anvil tools. No window, no GPU, no display libraries,
//! so it runs in a container or a script.
//!
//! Commands:
//!   anvil-cli card --name NAME --url URL [--out DIR] [--font FONT]
//!   anvil-cli kettle [--variant plain|gated|mold] [--out DIR]
//!   anvil-cli fonts
//!   anvil-cli --help

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "anvil-cli: headless Anvil CAD tools

USAGE:
    anvil-cli card --name NAME --url URL [--out DIR] [--font FONT]
    anvil-cli kettle [--variant plain|gated|mold] [--out DIR]
    anvil-cli fonts
    anvil-cli --help

COMMANDS:
    card     Make a 3D-printable business card with a name and a QR code.
             Writes business_card.stl, business_card.3mf, and
             business_card.anvil to DIR (default: current directory).
    kettle   Build the cast iron kettle sample (docs/KETTLE.md). Writes one
             STL per part, a 3MF, and the .anvil document. The gated
             variant also writes a Truchas case (cavity.stl, casting.inp).
    fonts    List the bundled fonts usable with --font.

OPTIONS for kettle:
    --variant V    plain (default), gated (with sprue, runner, riser), or
                   mold (pattern halves, core, core box)
    --out DIR      Output folder (created if missing)

OPTIONS for card:
    --name NAME    Name printed on the card
    --url URL      Link encoded in the QR code, for example a LinkedIn profile
    --out DIR      Output folder (created if missing)
    --font FONT    Bundled font name, for example \"Archivo Black\"
";

/// Parsed `card` options.
#[derive(Debug, PartialEq)]
struct CardArgs {
    name: String,
    url: String,
    out: PathBuf,
    font: Option<String>,
}

fn parse_card(args: &[String]) -> Result<CardArgs, String> {
    let mut name = None;
    let mut url = None;
    let mut out = PathBuf::from(".");
    let mut font = None;
    let mut i = 0;
    while i < args.len() {
        let flag = args[i].as_str();
        let value = || args.get(i + 1).cloned().ok_or_else(|| format!("{flag} needs a value"));
        match flag {
            "--name" => name = Some(value()?),
            "--url" => url = Some(value()?),
            "--out" => out = PathBuf::from(value()?),
            "--font" => font = Some(value()?),
            other => return Err(format!("unknown option {other}")),
        }
        i += 2;
    }
    Ok(CardArgs { name: name.ok_or("missing --name")?, url: url.ok_or("missing --url")?, out, font })
}

fn run_card(a: &CardArgs) -> Result<(), String> {
    use anvil_feature::features::emboss::TextFeature;
    let mut doc = anvil_io::business_card(&a.name, &a.url);
    if let Some(font) = &a.font {
        let source = if font.starts_with("builtin:") || font.contains('/') || font.contains('\\') {
            font.clone()
        } else {
            format!("builtin:{font}")
        };
        anvil_feature::fonts::load(&source)?;
        let idx = doc.features.iter().position(|f| f.feature.kind() == "text").ok_or("the card has no text feature")?;
        doc.edit_feature(idx, |f| {
            if let Some(t) = f.downcast_mut::<TextFeature>() {
                t.font_path = source.clone();
            }
        });
    }
    for (i, f) in doc.features.iter().enumerate() {
        if let Some(e) = &f.error {
            return Err(format!("feature {i} ({}): {e}", f.feature.name()));
        }
        if let Some(note) = f.output.as_ref().and_then(|o| o.note.as_ref()) {
            println!("{}: {note}", f.feature.name());
        }
    }
    std::fs::create_dir_all(&a.out).map_err(|e| format!("{}: {e}", a.out.display()))?;
    let mesh = anvil_io::document_mesh(&doc);
    anvil_io::write_stl(&mesh, &a.out.join("business_card.stl")).map_err(|e| e.to_string())?;
    let parts = anvil_io::write_3mf(&doc, &a.out.join("business_card.3mf")).map_err(|e| e.to_string())?;
    anvil_io::save_document(&doc, &a.out.join("business_card.anvil")).map_err(|e| e.to_string())?;
    println!(
        "Wrote business_card.stl, business_card.3mf ({parts} parts), and business_card.anvil to {} ({} triangles)",
        a.out.display(),
        mesh.triangle_count()
    );
    Ok(())
}

/// Parsed `kettle` options.
#[derive(Debug, PartialEq)]
struct KettleArgs {
    variant: String,
    out: PathBuf,
}

fn parse_kettle(args: &[String]) -> Result<KettleArgs, String> {
    let mut a = KettleArgs { variant: "plain".into(), out: PathBuf::from(".") };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--variant" => {
                a.variant = args.get(i + 1).ok_or("--variant needs a value")?.clone();
                i += 2;
            }
            "--out" => {
                a.out = PathBuf::from(args.get(i + 1).ok_or("--out needs a value")?);
                i += 2;
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    if !["plain", "gated", "mold"].contains(&a.variant.as_str()) {
        return Err(format!("unknown variant {}; use plain, gated, or mold", a.variant));
    }
    Ok(a)
}

fn run_kettle(a: &KettleArgs) -> Result<(), String> {
    let doc = match a.variant.as_str() {
        "gated" => anvil_io::kettle::kettle_gated(),
        "mold" => anvil_io::kettle::kettle_mold(),
        _ => anvil_io::kettle::kettle(),
    };
    for (i, f) in doc.features.iter().enumerate() {
        if let Some(e) = &f.error {
            return Err(format!("feature {i} ({}): {e}", f.feature.name()));
        }
        if let Some(note) = f.output.as_ref().and_then(|o| o.note.as_ref()) {
            println!("{}: {note}", f.feature.name());
        }
    }
    std::fs::create_dir_all(&a.out).map_err(|e| format!("{}: {e}", a.out.display()))?;
    let stem = format!("kettle_{}", a.variant);
    let files = anvil_io::write_stl_parts(&doc, &a.out.join(format!("{stem}.stl"))).map_err(|e| e.to_string())?;
    let parts = anvil_io::write_3mf(&doc, &a.out.join(format!("{stem}.3mf"))).map_err(|e| e.to_string())?;
    anvil_io::save_document(&doc, &a.out.join(format!("{stem}.anvil"))).map_err(|e| e.to_string())?;
    println!("Wrote {} STL parts, {stem}.3mf ({parts} parts), and {stem}.anvil to {}", files.len(), a.out.display());
    if a.variant == "gated" {
        let alloy = anvil_feature::features::casting::ALLOYS[0];
        let dir = a.out.join("truchas");
        let written =
            anvil_io::casting::write_truchas_case(&doc, 15, &alloy, 1400.0, 1.5, &dir).map_err(|e| e.to_string())?;
        println!("Wrote a Truchas case ({} files) to {}", written.len(), dir.display());
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(|s| s.as_str()) {
        None | Some("--help") | Some("-h") | Some("help") => {
            print!("{USAGE}");
            Ok(())
        }
        Some("fonts") => {
            for (name, _) in anvil_feature::fonts::BUILTIN {
                println!("{name}");
            }
            Ok(())
        }
        Some("card") => parse_card(&args[1..]).and_then(|a| run_card(&a)),
        Some("kettle") => parse_kettle(&args[1..]).and_then(|a| run_kettle(&a)),
        Some(other) => Err(format!("unknown command {other}\n\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn parses_kettle_options() {
        let a = parse_kettle(&["--variant".into(), "gated".into(), "--out".into(), "o".into()]).unwrap();
        assert_eq!(a, KettleArgs { variant: "gated".into(), out: PathBuf::from("o") });
        assert!(parse_kettle(&["--variant".into(), "cup".into()]).is_err());
    }

    #[test]
    fn parses_card_options() {
        let a = parse_card(&s(&["--name", "Your Name", "--url", "https://example.com", "--out", "/out"])).unwrap();
        assert_eq!(a.name, "Your Name");
        assert_eq!(a.out, PathBuf::from("/out"));
        assert!(parse_card(&s(&["--name", "X"])).unwrap_err().contains("--url"));
        assert!(parse_card(&s(&["--name"])).unwrap_err().contains("needs a value"));
        assert!(parse_card(&s(&["--bogus", "1"])).unwrap_err().contains("unknown option"));
    }

    #[test]
    fn card_command_writes_files() {
        let dir = std::env::temp_dir().join("anvil_cli_card_test");
        let _ = std::fs::remove_dir_all(&dir);
        let a = CardArgs {
            name: "Your Name".into(),
            url: "https://example.com".into(),
            out: dir.clone(),
            font: Some("Liberation Sans Bold".into()),
        };
        run_card(&a).unwrap();
        for f in ["business_card.stl", "business_card.3mf", "business_card.anvil"] {
            assert!(dir.join(f).metadata().unwrap().len() > 100, "{f}");
        }
        let bad = CardArgs { font: Some("No Such Font".into()), ..a };
        assert!(run_card(&bad).is_err());
    }
}
