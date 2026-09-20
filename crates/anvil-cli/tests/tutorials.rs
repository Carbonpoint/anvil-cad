//! Run the commands from docs/tutorials with tiny inputs, so a tutorial
//! that stops working fails CI instead of the reader.
//!
//! Every command here appears in a tutorial. The sizes and the voxel
//! sizes are shrunk with `--set` and coarse `--res` so the whole file
//! runs in seconds; the tutorials use the finer numbers they quote.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The repository root: tutorial documents name their CSV and VTK files
/// relative to it, and the tutorials tell the reader to run from there.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn out_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("anvil_tut_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Run anvil-cli from the repository root and return its stdout.
fn cli(args: &[&str]) -> String {
    let out =
        Command::new(env!("CARGO_BIN_EXE_anvil-cli")).current_dir(root()).args(args).output().expect("run anvil-cli");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        out.status.success(),
        "anvil-cli {args:?} failed\nstdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    stdout
}

/// Total volume of the bodies of one feature in a `run` report.
fn feature_volume(report: &Path, index: usize) -> f64 {
    let text = std::fs::read_to_string(report).expect("report");
    let v: serde_json::Value = serde_json::from_str(&text).expect("report json");
    v["features"][index]["bodies"]
        .as_array()
        .expect("bodies")
        .iter()
        .map(|b| b["volume_mm3"].as_f64().unwrap_or(0.0))
        .sum()
}

fn report_errors(report: &Path) -> u64 {
    let text = std::fs::read_to_string(report).expect("report");
    let v: serde_json::Value = serde_json::from_str(&text).expect("report json");
    v["errors"].as_u64().unwrap_or(1)
}

/// Tutorial 2 and 6: a body from a document becomes an STL, the STL is
/// filled with a sheet lattice, and a too thin wall is refused.
#[test]
fn tutorial_tpms_lattice_from_an_stl() {
    let dir = out_dir("tpms");
    let d = dir.to_str().unwrap();
    let plate = dir.join("plate.stl");
    cli(&[
        "run",
        "docs/tutorials/files/plate.anvil",
        "--set",
        "plate_x=24",
        "--set",
        "plate_y=16",
        "--set",
        "plate_z=12",
        "--formats",
        "stl",
        "--out",
        d,
    ]);
    assert!(plate.exists(), "plate.stl");
    assert!(
        (feature_volume(&dir.join("report.json"), 0) - 24.0 * 16.0 * 12.0).abs() < 1e-6,
        "the plate is 24 x 16 x 12 mm"
    );

    let note = cli(&[
        "lattice",
        "--stl",
        plate.to_str().unwrap(),
        "--kind",
        "gyroid",
        "--cell",
        "6",
        "--wall",
        "1.6",
        "--skin",
        "1.0",
        "--res",
        "0.8",
        "--out",
        d,
    ]);
    assert!(note.contains("gyroid lattice"), "{note}");
    assert!(note.contains("percent of the solid volume"), "{note}");
    for ext in ["stl", "3mf", "step"] {
        let p = dir.join(format!("plate_lattice.{ext}"));
        assert!(p.metadata().map(|m| m.len() > 100).unwrap_or(false), "{}", p.display());
    }

    // The guard the tutorial quotes: a wall thinner than two voxels.
    let bad = Command::new(env!("CARGO_BIN_EXE_anvil-cli"))
        .current_dir(root())
        .args(["lattice", "--stl", plate.to_str().unwrap(), "--wall", "0.6", "--res", "0.8", "--out", d])
        .output()
        .expect("run anvil-cli");
    assert!(!bad.status.success(), "a 0.6 mm wall at 0.8 mm voxels must be refused");
    let err = String::from_utf8_lossy(&bad.stderr);
    assert!(err.contains("thinner than two voxels"), "{err}");

    std::fs::remove_dir_all(&dir).ok();
}

/// Tutorial 3: a beam lattice written as a 3MF beam lattice.
#[test]
fn tutorial_beam_lattice_to_3mf() {
    let dir = out_dir("beams");
    let d = dir.to_str().unwrap();
    let plate = dir.join("plate.stl");
    cli(&[
        "run",
        "docs/tutorials/files/plate.anvil",
        "--set",
        "plate_x=24",
        "--set",
        "plate_y=16",
        "--set",
        "plate_z=12",
        "--formats",
        "stl",
        "--out",
        d,
    ]);
    let mut last = 0usize;
    for kind in ["cubic", "bcc", "octet", "kelvin"] {
        let note = cli(&[
            "beams",
            "--stl",
            plate.to_str().unwrap(),
            "--kind",
            kind,
            "--cell",
            "6",
            "--radius",
            "0.8",
            "--res",
            "0.8",
            "--out",
            d,
        ]);
        assert!(note.contains("beams on"), "{note}");
        let words: Vec<&str> = note.split_whitespace().collect();
        let at = words.iter().position(|w| *w == "beams").unwrap_or_else(|| panic!("beam count in {note}"));
        let beams: usize = words[at - 1].parse().unwrap_or_else(|_| panic!("beam count in {note}"));
        assert!(beams > 0, "{kind}: {note}");
        last = beams;
        assert!(dir.join("plate_beams.3mf").metadata().unwrap().len() > 100);
    }
    assert!(last > 0);

    std::fs::remove_dir_all(&dir).ok();
}

/// Tutorial 4: thickness graded along an axis, cell size graded along an
/// axis, and thickness graded by distance to another body.
#[test]
fn tutorial_grading() {
    let dir = out_dir("grade");
    let d = dir.to_str().unwrap();
    let plate = dir.join("plate.stl");
    cli(&[
        "run",
        "docs/tutorials/files/plate.anvil",
        "--set",
        "plate_x=24",
        "--set",
        "plate_y=16",
        "--set",
        "plate_z=12",
        "--formats",
        "stl",
        "--out",
        d,
    ]);
    let note = cli(&[
        "lattice",
        "--stl",
        plate.to_str().unwrap(),
        "--kind",
        "octet",
        "--cell",
        "6",
        "--wall",
        "1.6",
        "--wall-end",
        "2.4",
        "--grade",
        "z",
        "--skin",
        "1.0",
        "--res",
        "0.8",
        "--out",
        d,
    ]);
    assert!(note.contains("graded along z"), "{note}");

    // Cell size grading is a document parameter, not a CLI flag.
    cli(&[
        "run",
        "docs/tutorials/files/graded_cell.anvil",
        "--set",
        "plate_x=24",
        "--set",
        "plate_y=12",
        "--set",
        "plate_z=12",
        "--set",
        "res=1.0",
        "--set",
        "wall=2",
        "--set",
        "cell_near=5",
        "--set",
        "cell_far=12",
        "--formats",
        "stl",
        "--outputs",
        dir.join("graded.json").to_str().unwrap(),
        "--out",
        d,
    ]);
    assert_eq!(report_errors(&dir.join("graded.json")), 0);
    assert!(feature_volume(&dir.join("graded.json"), 1) > 0.0, "the graded lattice has a volume");

    // Grade by distance to a second body: the bracket document.
    cli(&[
        "run",
        "docs/tutorials/files/bracket.anvil",
        "--set",
        "plate_x=24",
        "--set",
        "plate_y=16",
        "--set",
        "plate_z=8",
        "--set",
        "cell=6",
        "--set",
        "wall=2.4",
        "--set",
        "wall_end=2",
        "--set",
        "skin=2.0",
        "--set",
        "res=1.0",
        "--formats",
        "stl",
        "--outputs",
        dir.join("bracket.json").to_str().unwrap(),
        "--out",
        d,
    ]);
    assert_eq!(report_errors(&dir.join("bracket.json")), 0);
    let v = feature_volume(&dir.join("bracket.json"), 2);
    let solid = 24.0 * 16.0 * 8.0;
    assert!(v > 0.1 * solid && v < solid, "ribbed plate volume {v} of {solid}");

    std::fs::remove_dir_all(&dir).ok();
}

/// Tutorial 5: a CSV point map drives wall thickness, and a VTK density
/// grid becomes a body.
#[test]
fn tutorial_fields_from_data() {
    let dir = out_dir("fields");
    let d = dir.to_str().unwrap();
    cli(&[
        "run",
        "docs/tutorials/files/stress_plate.anvil",
        "--set",
        "plate_x=24",
        "--set",
        "plate_y=14",
        "--set",
        "plate_z=12",
        "--set",
        "res=1.2",
        "--set",
        "wall_low=2.4",
        "--set",
        "wall_high=3",
        "--formats",
        "stl",
        "--outputs",
        dir.join("stress.json").to_str().unwrap(),
        "--out",
        d,
    ]);
    assert_eq!(report_errors(&dir.join("stress.json")), 0);
    assert!(feature_volume(&dir.join("stress.json"), 1) > 0.0);

    cli(&[
        "run",
        "docs/tutorials/files/density.anvil",
        "--set",
        "res=1.6",
        "--formats",
        "stl",
        "--outputs",
        dir.join("density.json").to_str().unwrap(),
        "--out",
        d,
    ]);
    assert_eq!(report_errors(&dir.join("density.json")), 0);
    // The grid is a 42 x 18 x 18 mm bar with a hole; the body is well
    // inside that box and not empty.
    let v = feature_volume(&dir.join("density.json"), 0);
    assert!(v > 2000.0 && v < 42.0 * 18.0 * 18.0, "density body volume {v}");

    std::fs::remove_dir_all(&dir).ok();
}

/// Tutorial 6: slices straight from the field, with areas and perimeters.
#[test]
fn tutorial_slices_from_the_field() {
    let dir = out_dir("slices");
    let d = dir.to_str().unwrap();
    let plate = dir.join("plate.stl");
    cli(&[
        "run",
        "docs/tutorials/files/plate.anvil",
        "--set",
        "plate_x=24",
        "--set",
        "plate_y=16",
        "--set",
        "plate_z=12",
        "--formats",
        "stl",
        "--out",
        d,
    ]);
    let note = cli(&["slice", "--stl", plate.to_str().unwrap(), "--layer", "3", "--res", "0.8", "--out", d]);
    assert!(note.contains("layers"), "{note}");
    let text = std::fs::read_to_string(dir.join("layers.json")).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let layers = v["layers"].as_array().unwrap();
    assert_eq!(layers.len(), 4, "12 mm at a 3 mm layer");
    for l in layers {
        let area = l["area_mm2"].as_f64().unwrap();
        // Every layer of a 24 x 16 mm plate is close to 384 mm2.
        assert!((area - 384.0).abs() < 20.0, "layer area {area}");
        assert!(dir.join(l["file"].as_str().unwrap()).exists());
    }

    std::fs::remove_dir_all(&dir).ok();
}

/// Tutorial 7: the aircraft wing, written and then rerun with changed
/// expressions, the loop the tutorial ends on.
#[test]
fn tutorial_aircraft_run_loop() {
    let dir = out_dir("plane");
    let d = dir.to_str().unwrap();
    cli(&["aircraft", "--out", d]);
    let doc = dir.join("plane.anvil");
    assert!(doc.exists());
    // One rerun with changed expressions: the loop the tutorial ends on.
    // The `aircraft` command alone rebuilds the wing, so one pass here
    // keeps the check under the runtime budget.
    let report = dir.join("plane_taper.json");
    cli(&[
        "run",
        doc.to_str().unwrap(),
        "--set",
        "taper=0.35",
        "--set",
        "resolution=8",
        "--formats",
        "stl",
        "--outputs",
        report.to_str().unwrap(),
        "--out",
        d,
    ]);
    assert_eq!(report_errors(&report), 0);
    let text = std::fs::read_to_string(&report).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let note = v["features"][0]["note"].as_str().unwrap_or("");
    assert!(note.contains("L/D"), "the aircraft note reports lift to drag: {note}");
    assert_eq!(v["expressions"]["taper"]["value"].as_f64(), Some(0.35));
    std::fs::remove_dir_all(&dir).ok();
}
