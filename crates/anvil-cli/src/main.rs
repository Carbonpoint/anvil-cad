//! anvil-cli: headless Anvil tools. No window, no GPU, no display libraries,
//! so it runs in a container or a script.
//!
//! Commands:
//!   anvil-cli card --name NAME --url URL [--out DIR] [--font FONT]
//!   anvil-cli kettle [--variant plain|gated|mold] [--out DIR]
//!   anvil-cli lattice --stl IN [--kind gyroid] [--cell 8] [--wall 1.2] [--skin 1.2] [--res 0.4] [--out DIR]
//!   anvil-cli fonts
//!   anvil-cli --help

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "anvil-cli: headless Anvil CAD tools

USAGE:
    anvil-cli card --name NAME --url URL [--out DIR] [--font FONT]
    anvil-cli kettle [--variant plain|gated|mold] [--out DIR]
    anvil-cli lattice --stl IN [options] [--out DIR]
    anvil-cli fonts
    anvil-cli --help

COMMANDS:
    card     Make a 3D-printable business card with a name and a QR code.
             Writes business_card.stl, business_card.3mf, and
             business_card.anvil to DIR (default: current directory).
    kettle   Build the cast iron kettle sample (docs/KETTLE.md). Writes one
             STL per part, a 3MF, and the .anvil document. The gated
             variant also writes a Truchas case (cavity.stl, casting.inp).
    lattice  Fill a closed STL with a lattice under a skin and write the
             result as STL, 3MF, and STEP (the field driven tools).
    fonts    List the bundled fonts usable with --font.

OPTIONS for kettle:
    --variant V    plain (default), gated (with sprue, runner, riser), or
                   mold (pattern halves, core, core box)
    --out DIR      Output folder (created if missing)

OPTIONS for lattice:
    --stl IN       Closed mesh to fill (millimetres)
    --kind K       gyroid (default), schwarz, diamond, cubic, bcc, octet, kelvin
    --cell C       Cell size, default 8
    --wall W       Sheet or beam thickness, default 1.2
    --wall-end W2  Thickness at the far end of the grade (0 = same)
    --grade G      none (default), x, y, z, or radial
    --conform C    none (default) or cylinder
    --skin S       Solid skin thickness, default 1.2 (0 = none)
    --res R        Voxel size, default 0.4
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

/// Parsed `lattice` options.
#[derive(Debug, PartialEq)]
struct LatticeArgs {
    stl: PathBuf,
    kind: String,
    cell: String,
    wall: String,
    wall_end: String,
    grade: String,
    conform: String,
    skin: String,
    res: String,
    out: PathBuf,
}

fn parse_lattice(args: &[String]) -> Result<LatticeArgs, String> {
    let mut a = LatticeArgs {
        stl: PathBuf::new(),
        kind: "gyroid".into(),
        cell: "8".into(),
        wall: "1.2".into(),
        wall_end: "0".into(),
        grade: "none".into(),
        conform: "none".into(),
        skin: "1.2".into(),
        res: "0.4".into(),
        out: PathBuf::from("."),
    };
    let mut i = 0;
    while i < args.len() {
        let value = || args.get(i + 1).cloned().ok_or(format!("{} needs a value", args[i]));
        match args[i].as_str() {
            "--stl" => a.stl = PathBuf::from(value()?),
            "--kind" => a.kind = value()?,
            "--cell" => a.cell = value()?,
            "--wall" => a.wall = value()?,
            "--wall-end" => a.wall_end = value()?,
            "--grade" => a.grade = value()?,
            "--conform" => a.conform = value()?,
            "--skin" => a.skin = value()?,
            "--res" => a.res = value()?,
            "--out" => a.out = PathBuf::from(value()?),
            other => return Err(format!("unknown option {other}")),
        }
        i += 2;
    }
    if a.stl.as_os_str().is_empty() {
        return Err("--stl is required".into());
    }
    Ok(a)
}

fn run_lattice(a: &LatticeArgs) -> Result<(), String> {
    use anvil_feature::features::lattice::LatticeFillFeature;
    use anvil_feature::features::solid_extra::MeshFeature;
    let mut doc = anvil_feature::Document::new("lattice");
    doc.add_feature(Box::new(MeshFeature { path: a.stl.display().to_string(), scale: "1".into() }));
    doc.add_feature(Box::new(LatticeFillFeature {
        body: 0,
        lattice: a.kind.clone(),
        cell: a.cell.clone(),
        wall: a.wall.clone(),
        wall_end: a.wall_end.clone(),
        grade: a.grade.clone(),
        conform: a.conform.clone(),
        skin: a.skin.clone(),
        resolution: a.res.clone(),
    }));
    for (i, f) in doc.features.iter().enumerate() {
        if let Some(e) = &f.error {
            return Err(format!("feature {i} ({}): {e}", f.feature.name()));
        }
        if let Some(note) = f.output.as_ref().and_then(|o| o.note.as_ref()) {
            println!("{}: {note}", f.feature.name());
        }
    }
    std::fs::create_dir_all(&a.out).map_err(|e| format!("{}: {e}", a.out.display()))?;
    let stem = a.stl.file_stem().and_then(|s| s.to_str()).unwrap_or("part");
    for fmt in [anvil_io::export::Format::Stl, anvil_io::export::Format::ThreeMf, anvil_io::export::Format::Step] {
        let p = a.out.join(format!("{stem}_lattice.{}", fmt.extension()));
        println!("{}", anvil_io::export::write(&doc, fmt, &p).map_err(|e| e.to_string())?);
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
        Some("lattice") => parse_lattice(&args[1..]).and_then(|a| run_lattice(&a)),
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
    fn parses_lattice_options() {
        let a = parse_lattice(&s(&["--stl", "p.stl", "--kind", "octet", "--grade", "z", "--wall-end", "2"])).unwrap();
        assert_eq!(a.kind, "octet");
        assert_eq!(a.grade, "z");
        assert_eq!(a.wall_end, "2");
        assert!(parse_lattice(&s(&["--kind", "octet"])).is_err(), "stl is required");
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
