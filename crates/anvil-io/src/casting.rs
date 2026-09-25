//! Casting exports: the mold cavity as one STL, and a Truchas input deck
//! template filled with the alloy data, so a fill and freeze run can
//! start from an Anvil document. See docs/research/casting_simulation.md
//! for why Truchas is the first target and what its deck needs.

use crate::IoError;
use anvil_feature::features::casting::Alloy;
use anvil_feature::Document;
use anvil_kernel::mesh::TriMesh;
use std::path::{Path, PathBuf};

/// Feature kinds that are part of the pour: the metal fills them too.
const GATING_KINDS: [&str; 3] = ["sprue", "runner", "riser"];

/// The mold cavity: the casting body plus every sprue, runner, and riser
/// body in the document, as one mesh.
pub fn cavity_mesh(doc: &Document, casting: usize) -> TriMesh {
    let mut out = TriMesh::default();
    for (fi, body) in doc.visible_bodies() {
        let kind = doc.features[fi].feature.kind();
        if fi == casting || GATING_KINDS.contains(&kind) {
            out.append(anvil_kernel::mesh::tessellate(body));
        }
    }
    out
}

/// The metal bodies of a document: the casting and every sprue, runner
/// and riser.
pub fn metal_bodies(doc: &Document, casting: usize) -> Vec<&anvil_kernel::Solid> {
    doc.visible_bodies()
        .into_iter()
        .filter(|(fi, _)| *fi == casting || GATING_KINDS.contains(&doc.features[*fi].feature.kind()))
        .map(|(_, b)| b)
        .collect()
}

/// A voxel mesh of the mold for Truchas: block 1 is sand, block 2 the
/// empty cavity. The mesh top is the cup rim, so the cup's top faces lie
/// on the outside of the mesh: the middle half is side set 1, the pour
/// inlet, and the rest side set 3, open to the room. Every other outside
/// face is side set 2. `sand` is the wall of sand kept
/// round the cavity, in mm. Returns the mesh and the cavity volume and
/// inlet area in mm3 and mm2.
pub fn voxel_mold(
    metal: &[&anvil_kernel::Solid],
    voxel: f64,
    sand: f64,
) -> Result<(crate::exodus::VoxelMesh, f64, f64), IoError> {
    use crate::exodus::{hex_side, VoxelMesh};
    let err = |m: &str| IoError::Import(m.to_string());
    if voxel.is_nan() || voxel <= 0.0 {
        return Err(err("the voxel size must be above 0"));
    }
    let mut lo = anvil_math::DVec3::splat(f64::INFINITY);
    let mut hi = anvil_math::DVec3::splat(f64::NEG_INFINITY);
    for b in metal {
        let bb = b.bounds();
        lo = lo.min(bb.min);
        hi = hi.max(bb.max);
    }
    if !lo.is_finite() {
        return Err(err("no casting body"));
    }
    let pad = (sand / voxel).ceil().max(2.0) * voxel;
    let lo = lo - anvil_math::DVec3::splat(pad);
    // No sand over the cup: the mesh ends at the rim.
    let hi = hi + anvil_math::DVec3::new(pad, pad, 0.0);
    let n = [
        ((hi.x - lo.x) / voxel).ceil() as usize,
        ((hi.y - lo.y) / voxel).ceil() as usize,
        ((hi.z - lo.z) / voxel).round().max(1.0) as usize,
    ];
    // Put the top of the grid on the rim exactly.
    let lo = anvil_math::DVec3::new(lo.x, lo.y, hi.z - n[2] as f64 * voxel);
    let cells = n[0] * n[1] * n[2];
    if cells > 20_000_000 {
        return Err(err("too many voxels; use a larger voxel size"));
    }
    let mut cavity = vec![false; cells];
    for b in metal {
        let m = anvil_kernel::mesh::tessellate(b);
        let tris: Vec<[anvil_math::DVec3; 3]> = m
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| [m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]])
            .collect();
        for (q, inside) in anvil_implicit::sampled::inside_grid(&tris, lo, voxel, n).into_iter().enumerate() {
            cavity[q] |= inside;
        }
    }
    let idx = |i: usize, j: usize, k: usize| (i * n[1] + j) * n[2] + k;
    let blocks: Vec<u8> = cavity.iter().map(|&c| if c { 2 } else { 1 }).collect();
    let (mut inlet, mut outside) = (Vec::new(), Vec::new());
    for i in 0..n[0] {
        for j in 0..n[1] {
            for k in 0..n[2] {
                let q = idx(i, j, k);
                let at = [i, j, k];
                for axis in 0..3 {
                    for high in [false, true] {
                        let edge = if high { at[axis] + 1 == n[axis] } else { at[axis] == 0 };
                        if !edge {
                            continue;
                        }
                        let side = hex_side(axis, high);
                        if axis == 2 && high && cavity[q] {
                            inlet.push((q, side));
                        } else {
                            outside.push((q, side));
                        }
                    }
                }
            }
        }
    }
    if inlet.len() < 2 {
        return Err(err("no cavity reaches the top of the mold: the sprue cup must be the highest metal"));
    }
    // The middle half of the cup top pours; the rest stays open to the
    // room, so void can leave and the pressure has a reference.
    let centre = |q: usize| {
        let (i, r) = (q / (n[1] * n[2]), q % (n[1] * n[2]));
        anvil_math::DVec2::new(i as f64, (r / n[2]) as f64)
    };
    let mid = inlet.iter().map(|(q, _)| centre(*q)).sum::<anvil_math::DVec2>() / inlet.len() as f64;
    inlet.sort_by(|a, b| (centre(a.0) - mid).length_squared().total_cmp(&(centre(b.0) - mid).length_squared()));
    let open = inlet.split_off(inlet.len().div_ceil(2));
    let v3 = voxel * voxel * voxel;
    let volume = cavity.iter().filter(|&&c| c).count() as f64 * v3;
    let inlet_area = inlet.len() as f64 * voxel * voxel;
    let mesh = VoxelMesh {
        lo,
        step: voxel,
        n,
        blocks,
        side_sets: vec![
            (1, "pour inlet".into(), inlet),
            (2, "mold outside".into(), outside),
            (3, "cup open top".into(), open),
        ],
    };
    Ok((mesh, volume, inlet_area))
}

/// Write `mesh.exo`, `casting.inp`, `cavity.stl` and `README.md` into
/// `dir`. `pour_temp` is in C, `pour_speed` in m/s down through the cup,
/// `voxel` in mm, `end_time` in s (0 runs a little past the fill).
#[allow(clippy::too_many_arguments)]
pub fn write_truchas_case(
    doc: &Document,
    casting: usize,
    alloy: &Alloy,
    pour_temp: f64,
    pour_speed: f64,
    voxel: f64,
    end_time: f64,
    dir: &Path,
) -> Result<Vec<PathBuf>, IoError> {
    std::fs::create_dir_all(dir)?;
    let stl = dir.join("cavity.stl");
    crate::write_stl(&cavity_mesh(doc, casting), &stl)?;
    let metal = metal_bodies(doc, casting);
    let (mesh, volume, inlet_area) = voxel_mold(&metal, voxel, 3.0 * voxel)?;
    let exo = dir.join("mesh.exo");
    std::fs::write(&exo, mesh.to_exodus(&doc.name))?;
    // Fill time: the cavity volume through the inlet at the pour speed.
    let fill_s = volume * 1e-9 / (inlet_area * 1e-6 * pour_speed.max(0.01));
    let end = if end_time > 0.0 { end_time } else { fill_s * 1.5 };
    let inp = dir.join("casting.inp");
    std::fs::write(&inp, truchas_deck(alloy, pour_temp, pour_speed, end))?;
    let readme = dir.join("README.md");
    std::fs::write(&readme, truchas_readme(alloy, volume * 1e-3, fill_s, mesh.blocks.len(), voxel))?;
    Ok(vec![exo, inp, stl, readme])
}

/// A Truchas input deck for `mesh.exo` from `voxel_mold`. Units are SI
/// (the mesh is in mm and scaled), temperatures in K. The namelists follow
/// the Truchas reference manual and its freezing-flow tests.
pub fn truchas_deck(alloy: &Alloy, pour_temp: f64, pour_speed: f64, end_s: f64) -> String {
    let tag = alloy.name.split_whitespace().next().unwrap_or("metal").to_lowercase();
    let latent_j_per_kg = alloy.latent_heat * 1e3;
    let pour_k = pour_temp + 273.15;
    let mold_k = 300.0;
    format!(
        r#"Truchas input deck written by Anvil: {name} poured at {pour_temp:.0} C into a
sand mold. mesh.exo is a voxel mesh in mm: block 1 sand, block 2 the empty
cavity; side set 1 is the pour inlet in the middle of the cup rim, side set
3 the rest of the cup rim, open to the room, side set 2 the rest of the
outside. Units SI, temperatures in K.

&MESH
  mesh_file = 'mesh.exo'
  coord_scale_factor = 0.001
/

&OUTPUTS
  output_t  = 0.0, {end_s:.3}
  output_dt = {out_dt:.4}
/

&PHYSICS
  materials = 'sand', '{tag}', 'VOID'
  flow = .true.
  heat_transport = .true.
  body_force_density = 0.0, 0.0, -9.81
/

&FLOW
  inviscid = .true.
  courant_number = 0.4
  vol_track_subcycles = 2
/

&FLOW_PRESSURE_SOLVER
  rel_tol = 0.0
  abs_tol = 1.0e-10
  max_ds_iter = 50
  max_amg_iter = 25
  krylov_method = 'cg'
/

Void in the cavity needs the non-adaptive stepping, as in Truchas's
htvoid tests.
&DIFFUSION_SOLVER
  stepping_method       = 'Non-adaptive BDF1'
  cond_vfrac_threshold  = 1.0e-4
  residual_atol         = 1.0e-9
  residual_rtol         = 1.0e-5
  max_nlk_itr           = 50
  nlk_preconditioner    = 'hypre_amg'
  vfr_solve_tol         = 1.0e-6
/

&NUMERICS
  dt_init = 1.0e-4
  dt_min  = 1.0e-8
  dt_max  = 5.0e-3
  dt_grow = 1.1
/

The sand.
&BODY
  surface_name = 'from mesh file'
  mesh_material_number = 1
  material_name = 'sand'
  temperature = {mold_k:.1}
/

The empty cavity.
&BODY
  surface_name = 'from mesh file'
  mesh_material_number = 2
  material_name = 'VOID'
  temperature = {mold_k:.1}
/

Metal pours down through the middle of the cup top.
&FLOW_BC
  name = 'pour'
  face_set_ids = 1
  type = 'velocity'
  velocity = 0.0, 0.0, -{pour_speed:.3}
  inflow_material = '{tag}-liquid'
  inflow_temperature = {pour_k:.1}
/

The rest of the cup top is open to the room: void leaves through it while
the mold fills, and metal once it is full, so the pressure always has a
reference.
&FLOW_BC
  name = 'open cup'
  face_set_ids = 3
  type = 'pressure'
  pressure = 0.0
/

&THERMAL_BC
  name = 'pour'
  face_set_ids = 1, 3
  type = 'temperature'
  temp = {pour_k:.1}
/

The outside of the sand loses heat slowly to the room. Calibrate from one
pour.
&THERMAL_BC
  name = 'mold outside'
  face_set_ids = 2
  type = 'htc'
  htc = 10.0
  ambient_temp = {mold_k:.1}
/

Green sand, starting values.
&MATERIAL
  name = 'sand'
  density = 1600.0
  specific_heat = 1100.0
  conductivity = 0.7
/

&MATERIAL
  name = '{tag}'
  density = {rho_l:.0}
  specific_heat = {cp:.0}
  conductivity = {k:.1}
  phases = '{tag}-solid', '{tag}-liquid'
/

&PHASE
  name = '{tag}-liquid'
  is_fluid = T
/

&PHASE_CHANGE
  low_temp_phase  = '{tag}-solid'
  high_temp_phase = '{tag}-liquid'
  solidus_temp  = {solidus:.1}
  liquidus_temp = {liquidus:.1}
  latent_heat   = {latent:.0}
/
"#,
        name = alloy.name,
        end_s = end_s,
        out_dt = (end_s / 20.0).max(1e-3),
        tag = tag,
        rho_l = alloy.density_liquid,
        cp = alloy.specific_heat,
        k = alloy.conductivity,
        solidus = alloy.solidus + 273.15,
        liquidus = alloy.liquidus + 273.15,
        latent = latent_j_per_kg,
        mold_k = mold_k,
        pour_speed = pour_speed,
        pour_k = pour_k,
    )
}

fn truchas_readme(alloy: &Alloy, volume_cm3: f64, fill_s: f64, cells: usize, voxel: f64) -> String {
    format!(
        r#"# Truchas case from Anvil

Cavity {volume_cm3:.0} cm3 of {name}, fills in about {fill_s:.1} s at the
pour speed. mesh.exo has {cells} hex cells of {voxel} mm: block 1 sand,
block 2 the empty cavity; side set 1 is the inlet at the cup rim, side set 2
the rest of the outside.

Run it with `truchas casting.inp`, or on several cores with
`mpirun -np 8 truchas casting.inp`. Results go to `casting_output/` as an
HDF5 file; `write-xdmf.py` from Truchas turns them into a file ParaView
opens.

The material numbers are starting values from
docs/research/casting_simulation.md. Calibrate the mold heat transfer and
the pour speed from one real pour.
"#,
        name = alloy.name,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_feature::features::casting::ALLOYS;

    #[test]
    fn truchas_case_has_the_alloy_numbers() {
        let deck = truchas_deck(&ALLOYS[0], 1400.0, 1.5, 3.0);
        assert!(deck.contains("solidus_temp  = 1423."), "{deck}");
        assert!(deck.contains("latent_heat   = 280000"));
        assert!(deck.contains("inflow_temperature = 1673."));
        assert!(deck.contains("temp = 1673."), "a temperature condition takes temp");
    }

    #[test]
    fn a_plate_with_a_sprue_makes_a_mold_mesh_with_an_inlet() {
        use anvil_kernel::ops;
        use anvil_math::{DVec2, DVec3, Plane};
        // A 40 x 20 x 6 plate, and a 10 x 10 sprue standing 30 mm on it.
        let plate = ops::box_solid(DVec3::ZERO, DVec3::new(40.0, 20.0, 6.0)).unwrap();
        let sprue = ops::extrude(
            &Plane { origin: DVec3::new(0.0, 0.0, 6.0), ..Plane::XY },
            &[DVec2::new(0.0, 5.0), DVec2::new(10.0, 5.0), DVec2::new(10.0, 15.0), DVec2::new(0.0, 15.0)],
            30.0,
        )
        .unwrap();
        let (mesh, volume, inlet) = voxel_mold(&[&plate, &sprue], 2.0, 6.0).unwrap();
        assert!((volume - (4800.0 + 3000.0)).abs() < 1.0, "{volume}");
        // The sprue top is 10 x 10 mm, 25 voxels; the middle 13 pour.
        assert!((inlet - 52.0).abs() < 1e-9, "{inlet}");
        let bytes = mesh.to_exodus("plate");
        assert_eq!(&bytes[..4], b"CDF\x02");
    }
}
