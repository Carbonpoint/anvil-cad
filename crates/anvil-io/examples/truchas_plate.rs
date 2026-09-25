//! A small Truchas case to try the solver on: a 60 x 30 x 8 mm grey iron
//! plate fed by a 12 mm square sprue 40 mm tall.
//!
//! `cargo run -p anvil-io --example truchas_plate -- OUT_DIR [VOXEL_MM] [END_S]`

use anvil_kernel::ops;
use anvil_math::{DVec2, DVec3, Plane};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).cloned().unwrap_or_else(|| "truchas_plate".into()));
    let voxel: f64 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(2.0);
    let end: f64 = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let plate = ops::box_solid(DVec3::ZERO, DVec3::new(60.0, 30.0, 8.0)).unwrap();
    let sprue = ops::extrude(
        &Plane { origin: DVec3::new(0.0, 0.0, 8.0), ..Plane::XY },
        &[DVec2::new(2.0, 9.0), DVec2::new(14.0, 9.0), DVec2::new(14.0, 21.0), DVec2::new(2.0, 21.0)],
        40.0,
    )
    .unwrap();
    let alloy = anvil_feature::features::casting::ALLOYS[0];
    let (mesh, volume, inlet) =
        anvil_io::casting::voxel_mold(&[&plate, &sprue], voxel, (3.0 * voxel).max(30.0)).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("mesh.exo"), mesh.to_exodus("plate")).unwrap();
    let speed = 0.5;
    let fill = volume * 1e-9 / (inlet * 1e-6 * speed);
    let end = if end > 0.0 { end } else { fill * 1.5 };
    std::fs::write(out.join("casting.inp"), anvil_io::casting::truchas_deck(&alloy, 1400.0, speed, fill, end)).unwrap();
    std::fs::write(out.join("freeze.inp"), anvil_io::casting::truchas_freeze_deck(&alloy, 1400.0, 60.0)).unwrap();
    // Anvil's own estimate of where the metal freezes last, to set
    // beside the Truchas freeze.
    let a = anvil_feature::features::hot_spots::analyse(&[&plate, &sprue], &[], &[], None, 1.0).unwrap();
    println!(
        "Hot spots: the last metal to freeze is {:.1} mm from the mold at ({:.1}, {:.1}, {:.1})",
        a.d_max, a.at_max.x, a.at_max.y, a.at_max.z
    );
    println!(
        "{} cells, cavity {:.1} cm3, inlet {:.0} mm2, fills in {:.2} s, runs to {:.2} s",
        mesh.blocks.len(),
        volume * 1e-3,
        inlet,
        fill,
        end
    );
}
