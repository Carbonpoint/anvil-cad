//! anvil-cli: headless Anvil tools. No window, no GPU, no display libraries,
//! so it runs in a container or a script.
//!
//! Commands:
//!   anvil-cli card --name NAME --url URL [--out DIR] [--font FONT]
//!   anvil-cli kettle [--variant plain|gated|mold] [--out DIR]
//!   anvil-cli lattice --stl IN [--kind gyroid] [--cell 8] [--wall 1.2] [--skin 1.2] [--res 0.4] [--out DIR]
//!   anvil-cli aircraft [--out DIR]
//!   anvil-cli slice --stl IN [--layer 0.2] [--res 0.4] [--out DIR]
//!   anvil-cli run DOC.anvil [--set name=value]... [--inputs in.json] [--outputs out.json] [--out DIR] [--formats 3mf,stl]
//!                 [--openfoam --speed 20 --alpha 4 --sref M2 --cref M]
//!   anvil-cli fonts
//!   anvil-cli --help

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "anvil-cli: headless Anvil CAD tools

USAGE:
    anvil-cli card --name NAME --url URL [--out DIR] [--font FONT]
    anvil-cli kettle [--variant plain|gated|mold] [--out DIR]
    anvil-cli lattice --stl IN [options] [--out DIR]
    anvil-cli aircraft [--out DIR]
    anvil-cli slice --stl IN [--layer 0.2] [--res 0.4] [--out DIR]
    anvil-cli run DOC.anvil [--set name=value]... [--inputs in.json] [--outputs out.json] [--out DIR]
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
    aircraft Write plane.anvil: the Aircraft feature with its wing driven
             by expressions (span, root_chord, taper, sweep, dihedral,
             twist_tip, resolution), ready for `run --set` and the loop in
             docs/examples/wing_loop.py.
    slice    Sample a closed STL into a distance field and write one SVG
             per layer straight from the field, plus layers.json with the
             area and perimeter of each layer.
    run      Open a document, set named expressions from --set pairs or a
             JSON object, rebuild, export the bodies, and write a JSON
             report (expressions, feature notes, volumes, bounds, errors).
             This is the hook for design loops driven from a script.
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

OPTIONS for slice:
    --stl IN       Closed mesh to slice (millimetres)
    --layer H      Layer height, default 0.2
    --res R        Sampling voxel size, default 0.4
    --out DIR      Output folder (created if missing)

OPTIONS for run:
    --set N=V      Set expression N to V (a number or a formula); repeatable
    --inputs F     JSON object of expression names to values
    --outputs F    Where to write the JSON report (default: DIR/report.json)
    --formats L    Comma list of 3mf, 3mf-group, stl, stl-parts, step, obj,
                   ply, off, amf, gltf (default: 3mf-group,stl-parts)
    --out DIR      Output folder (created if missing)
    --openfoam     Also write an OpenFOAM case (simpleFoam, k omega SST,
                   force coefficients) for the visible bodies in DIR/openfoam
    --speed V      Free stream speed for the case, m/s (default 20)
    --alpha A      Angle of attack, degrees (default 4)
    --sref S       Reference area, m2 (default: y extent times cref)
    --cref C       Reference chord, m (default: a quarter of the x extent)

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
        ..Default::default()
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

/// Parsed `run` options.
#[derive(Debug, PartialEq)]
struct RunArgs {
    doc: PathBuf,
    sets: Vec<(String, String)>,
    inputs: Option<PathBuf>,
    outputs: Option<PathBuf>,
    formats: Vec<String>,
    out: PathBuf,
    openfoam: bool,
    speed: f64,
    alpha: f64,
    sref: Option<f64>,
    cref: Option<f64>,
}

fn parse_run(args: &[String]) -> Result<RunArgs, String> {
    let mut a = RunArgs {
        doc: PathBuf::new(),
        sets: Vec::new(),
        inputs: None,
        outputs: None,
        formats: vec!["3mf-group".into(), "stl-parts".into()],
        out: PathBuf::from("."),
        openfoam: false,
        speed: 20.0,
        alpha: 4.0,
        sref: None,
        cref: None,
    };
    let mut i = 0;
    while i < args.len() {
        let value = || args.get(i + 1).cloned().ok_or(format!("{} needs a value", args[i]));
        let number = || value()?.parse::<f64>().map_err(|e| format!("{}: {e}", args[i]));
        match args[i].as_str() {
            "--openfoam" => {
                a.openfoam = true;
                i += 1;
            }
            "--speed" => {
                a.speed = number()?;
                i += 2;
            }
            "--alpha" => {
                a.alpha = number()?;
                i += 2;
            }
            "--sref" => {
                a.sref = Some(number()?);
                i += 2;
            }
            "--cref" => {
                a.cref = Some(number()?);
                i += 2;
            }
            "--set" => {
                let v = value()?;
                let (n, e) = v.split_once('=').ok_or(format!("--set wants name=value, got {v}"))?;
                a.sets.push((n.trim().to_string(), e.trim().to_string()));
                i += 2;
            }
            "--inputs" => {
                a.inputs = Some(PathBuf::from(value()?));
                i += 2;
            }
            "--outputs" => {
                a.outputs = Some(PathBuf::from(value()?));
                i += 2;
            }
            "--formats" => {
                a.formats =
                    value()?.split(',').map(|f| f.trim().to_ascii_lowercase()).filter(|f| !f.is_empty()).collect();
                i += 2;
            }
            "--out" => {
                a.out = PathBuf::from(value()?);
                i += 2;
            }
            other if other.starts_with("--") => return Err(format!("unknown option {other}")),
            doc => {
                a.doc = PathBuf::from(doc);
                i += 1;
            }
        }
    }
    if a.doc.as_os_str().is_empty() {
        return Err("give the .anvil document to run".into());
    }
    Ok(a)
}

fn format_of(name: &str) -> Result<anvil_io::export::Format, String> {
    use anvil_io::export::Format;
    Ok(match name {
        "3mf" => Format::ThreeMf,
        "3mf-group" => Format::ThreeMfGroup,
        "stl" => Format::Stl,
        "stl-parts" => Format::StlParts,
        "step" | "stp" => Format::Step,
        "obj" => Format::Obj,
        "ply" => Format::Ply,
        "off" => Format::Off,
        "amf" => Format::Amf,
        "gltf" => Format::Gltf,
        other => return Err(format!("unknown format {other}")),
    })
}

fn run_document(a: &RunArgs) -> Result<(), String> {
    let mut doc = anvil_io::load_document(&a.doc).map_err(|e| format!("{}: {e}", a.doc.display()))?;
    let mut sets = a.sets.clone();
    if let Some(p) = &a.inputs {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", p.display()))?;
        let obj = v.as_object().ok_or("inputs must be a JSON object of name: value")?;
        for (k, val) in obj {
            let src = match val {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            sets.push((k.clone(), src));
        }
    }
    for (name, src) in &sets {
        doc.set_expression(name, src).map_err(|e| format!("expression {name} = {src}: {e}"))?;
    }
    std::fs::create_dir_all(&a.out).map_err(|e| format!("{}: {e}", a.out.display()))?;
    let stem = a.doc.file_stem().and_then(|s| s.to_str()).unwrap_or("part").to_string();
    let mut files = Vec::new();
    for f in &a.formats {
        let fmt = format_of(f)?;
        let path = a.out.join(format!("{stem}.{}", fmt.extension()));
        let note = anvil_io::export::write(&doc, fmt, &path).map_err(|e| e.to_string())?;
        println!("{note}");
        files.push(path.display().to_string());
    }
    let exprs: serde_json::Map<String, serde_json::Value> = doc
        .exprs
        .iter()
        .map(|p| (p.name.clone(), serde_json::json!({ "source": p.source, "value": p.value })))
        .collect();
    let features: Vec<serde_json::Value> = doc
        .features
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let bodies: Vec<serde_json::Value> = f
                .output
                .as_ref()
                .map(|o| {
                    o.bodies
                        .iter()
                        .map(|b| {
                            let bb = b.bounds();
                            serde_json::json!({
                                "volume_mm3": b.volume(),
                                "faces": b.faces.len(),
                                "open_edges": b.open_edge_report().0,
                                "min": [bb.min.x, bb.min.y, bb.min.z],
                                "max": [bb.max.x, bb.max.y, bb.max.z],
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            serde_json::json!({
                "index": i,
                "name": f.feature.name(),
                "kind": f.feature.kind(),
                "error": f.error.as_ref().map(|e| e.to_string()),
                "note": f.output.as_ref().and_then(|o| o.note.clone()),
                "mass_g": doc.mass_of(i),
                "bodies": bodies,
            })
        })
        .collect();
    if a.openfoam {
        let mut lo = anvil_math_bounds_lo();
        let mut hi = anvil_math_bounds_hi();
        for b in doc.bodies() {
            let bb = b.bounds();
            lo = [lo[0].min(bb.min.x), lo[1].min(bb.min.y), lo[2].min(bb.min.z)];
            hi = [hi[0].max(bb.max.x), hi[1].max(bb.max.y), hi[2].max(bb.max.z)];
        }
        let cref = a.cref.unwrap_or(0.25 * (hi[0] - lo[0]).max(1.0) * 0.001);
        let sref = a.sref.unwrap_or((hi[1] - lo[1]).max(1.0) * 0.001 * cref);
        let case = anvil_io::aero::AeroCase { speed: a.speed, alpha_deg: a.alpha, area_m2: sref, chord_m: cref };
        let dir = a.out.join("openfoam");
        let written = anvil_io::aero::write_openfoam_case(&doc, &case, &dir).map_err(|e| e.to_string())?;
        println!(
            "Wrote an OpenFOAM case ({} files) to {} (Aref {sref:.4} m2, cref {cref:.4} m)",
            written.len(),
            dir.display()
        );
        files.push(dir.display().to_string());
    }
    let errors = doc.features.iter().filter(|f| f.error.is_some()).count();
    let report = serde_json::json!({
        "document": a.doc.display().to_string(),
        "inputs": sets.iter().map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone()))).collect::<serde_json::Map<_, _>>(),
        "expressions": exprs,
        "features": features,
        "visible_bodies": doc.bodies().len(),
        "errors": errors,
        "files": files,
    });
    let out_path = a.outputs.clone().unwrap_or_else(|| a.out.join("report.json"));
    std::fs::write(&out_path, serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?)
        .map_err(|e| format!("{}: {e}", out_path.display()))?;
    println!("Wrote {} ({} features, {errors} errors)", out_path.display(), doc.features.len());
    if errors > 0 {
        return Err(format!("{errors} feature(s) failed; see {}", out_path.display()));
    }
    Ok(())
}

/// Parsed `slice` options.
#[derive(Debug, PartialEq)]
struct SliceArgs {
    stl: PathBuf,
    layer: f64,
    res: f64,
    out: PathBuf,
}

fn parse_slice(args: &[String]) -> Result<SliceArgs, String> {
    let mut a = SliceArgs { stl: PathBuf::new(), layer: 0.2, res: 0.4, out: PathBuf::from(".") };
    let mut i = 0;
    while i < args.len() {
        let value = || args.get(i + 1).cloned().ok_or(format!("{} needs a value", args[i]));
        match args[i].as_str() {
            "--stl" => a.stl = PathBuf::from(value()?),
            "--layer" => a.layer = value()?.parse().map_err(|e| format!("--layer: {e}"))?,
            "--res" => a.res = value()?.parse().map_err(|e| format!("--res: {e}"))?,
            "--out" => a.out = PathBuf::from(value()?),
            other => return Err(format!("unknown option {other}")),
        }
        i += 2;
    }
    if a.stl.as_os_str().is_empty() {
        return Err("--stl is required".into());
    }
    if a.layer <= 0.0 || a.res <= 0.0 {
        return Err("layer and res must be positive".into());
    }
    Ok(a)
}

fn run_slice(a: &SliceArgs) -> Result<(), String> {
    use anvil_implicit::slice::{area, contours, length, svg};
    use anvil_implicit::Sampled;
    let tris = anvil_feature::mesh_loader::load(&a.stl.display().to_string())?;
    if tris.is_empty() {
        return Err("no triangles".into());
    }
    let field = Sampled::from_triangles(&tris, a.res, 3);
    let mut lo = anvil_math::DVec3::splat(f64::INFINITY);
    let mut hi = anvil_math::DVec3::splat(f64::NEG_INFINITY);
    for t in &tris {
        for p in t {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
    }
    std::fs::create_dir_all(&a.out).map_err(|e| format!("{}: {e}", a.out.display()))?;
    let plo = anvil_math::DVec2::new(lo.x - a.res, lo.y - a.res);
    let phi = anvil_math::DVec2::new(hi.x + a.res, hi.y + a.res);
    let mut layers = Vec::new();
    let mut z = lo.z + 0.5 * a.layer;
    let mut n = 0usize;
    while z < hi.z {
        let loops = contours(&field, z, plo, phi, a.res);
        let total_area: f64 = loops.iter().map(|l| area(l)).sum();
        let perimeter: f64 = loops.iter().map(|l| length(l)).sum();
        let name = format!("layer_{n:04}.svg");
        std::fs::write(a.out.join(&name), svg(&loops, plo, phi)).map_err(|e| e.to_string())?;
        layers.push(serde_json::json!({ "z": z, "file": name, "loops": loops.len(), "area_mm2": total_area, "perimeter_mm": perimeter }));
        n += 1;
        z += a.layer;
    }
    let report = serde_json::json!({ "stl": a.stl.display().to_string(), "layer": a.layer, "layers": layers });
    std::fs::write(a.out.join("layers.json"), serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    println!("Wrote {n} layers to {}", a.out.display());
    Ok(())
}

fn run_aircraft(out: &Path) -> Result<(), String> {
    use anvil_feature::features::aircraft::AircraftFeature;
    let mut doc = anvil_feature::Document::new("plane");
    for (k, v) in [
        ("span", "180"),
        ("root_chord", "30"),
        ("taper", "0.5"),
        ("sweep", "8"),
        ("dihedral", "4"),
        ("twist_tip", "-1"),
        ("resolution", "1.0"),
    ] {
        doc.set_expression(k, v).map_err(|e| e.to_string())?;
    }
    doc.add_feature(Box::new(AircraftFeature {
        span: "span".into(),
        root_chord: "root_chord".into(),
        tip_chord: "root_chord * taper".into(),
        sweep: "sweep".into(),
        dihedral: "dihedral".into(),
        twist_tip: "twist_tip".into(),
        resolution: "resolution".into(),
        ..Default::default()
    }));
    if let Some(e) = &doc.features[0].error {
        return Err(e.to_string());
    }
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let path = out.join("plane.anvil");
    anvil_io::save_document(&doc, &path).map_err(|e| e.to_string())?;
    println!("Wrote {}", path.display());
    if let Some(n) = doc.features[0].output.as_ref().and_then(|o| o.note.as_ref()) {
        println!("{n}");
    }
    Ok(())
}

fn anvil_math_bounds_lo() -> [f64; 3] {
    [f64::INFINITY; 3]
}
fn anvil_math_bounds_hi() -> [f64; 3] {
    [f64::NEG_INFINITY; 3]
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
        Some("run") => parse_run(&args[1..]).and_then(|a| run_document(&a)),
        Some("slice") => parse_slice(&args[1..]).and_then(|a| run_slice(&a)),
        Some("aircraft") => {
            let out = match args.get(1).map(|s| s.as_str()) {
                Some("--out") => args.get(2).map(PathBuf::from).ok_or("--out needs a value".to_string()),
                None => Ok(PathBuf::from(".")),
                Some(other) => Err(format!("unknown option {other}")),
            };
            out.and_then(|o| run_aircraft(&o))
        }
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
    fn parses_run_options() {
        let a = parse_run(&s(&[
            "part.anvil",
            "--set",
            "span=200",
            "--set",
            "sweep = 5",
            "--formats",
            "stl,step",
            "--out",
            "o",
        ]))
        .unwrap();
        assert_eq!(a.doc, PathBuf::from("part.anvil"));
        assert_eq!(a.sets, vec![("span".to_string(), "200".to_string()), ("sweep".to_string(), "5".to_string())]);
        assert_eq!(a.formats, vec!["stl".to_string(), "step".to_string()]);
        assert!(parse_run(&s(&["--set", "a=1"])).is_err(), "document is required");
        assert!(parse_run(&s(&["p.anvil", "--set", "novalue"])).is_err());
    }

    #[test]
    fn run_sets_an_expression_and_writes_a_report() {
        use anvil_feature::features::primitives::BoxFeature;
        let dir = std::env::temp_dir().join(format!("anvil_run_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut doc = anvil_feature::Document::new("run");
        doc.set_expression("w", "10").unwrap();
        doc.add_feature(Box::new(BoxFeature {
            x: "0".into(),
            y: "0".into(),
            z: "0".into(),
            width: "w".into(),
            depth: "5".into(),
            height: "2".into(),
        }));
        let path = dir.join("box.anvil");
        anvil_io::save_document(&doc, &path).unwrap();
        let a = parse_run(&s(&[
            path.to_str().unwrap(),
            "--set",
            "w=20",
            "--formats",
            "stl",
            "--out",
            dir.to_str().unwrap(),
        ]))
        .unwrap();
        run_document(&a).unwrap();
        let report: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("report.json")).unwrap()).unwrap();
        let vol = report["features"][0]["bodies"][0]["volume_mm3"].as_f64().unwrap();
        assert!((vol - 200.0).abs() < 1e-6, "volume with w = 20: {vol}");
        assert_eq!(report["expressions"]["w"]["value"].as_f64().unwrap(), 20.0);
        assert!(dir.join("box.stl").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parses_slice_options() {
        let a = parse_slice(&s(&["--stl", "p.stl", "--layer", "0.3"])).unwrap();
        assert_eq!(a.layer, 0.3);
        assert!(parse_slice(&s(&["--layer", "0.3"])).is_err());
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
