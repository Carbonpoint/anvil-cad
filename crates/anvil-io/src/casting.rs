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

/// Write `cavity.stl`, `casting.inp`, and `README.md` into `dir`. Returns
/// the paths written. `pour_temp` is in C, `pour_speed` in m/s at the
/// sprue inlet.
pub fn write_truchas_case(
    doc: &Document,
    casting: usize,
    alloy: &Alloy,
    pour_temp: f64,
    pour_speed: f64,
    dir: &Path,
) -> Result<Vec<PathBuf>, IoError> {
    std::fs::create_dir_all(dir)?;
    let mesh = cavity_mesh(doc, casting);
    let stl = dir.join("cavity.stl");
    crate::write_stl(&mesh, &stl)?;
    let volume_cm3 = mesh.signed_volume() * 1e-3;
    let deck = truchas_deck(alloy, pour_temp, pour_speed, volume_cm3);
    let inp = dir.join("casting.inp");
    std::fs::write(&inp, deck)?;
    let readme = dir.join("README.md");
    std::fs::write(&readme, truchas_readme(alloy, volume_cm3))?;
    Ok(vec![stl, inp, readme])
}

/// A Truchas namelist deck with the alloy data filled in. The mesh, the
/// face sets, and the time step are the parts to check against the
/// Truchas reference manual before a run; they depend on the mesh you
/// make from `cavity.stl`.
pub fn truchas_deck(alloy: &Alloy, pour_temp: f64, pour_speed: f64, volume_cm3: f64) -> String {
    let tag = alloy.name.split_whitespace().next().unwrap_or("metal").to_lowercase();
    let latent_j_per_kg = alloy.latent_heat * 1e3;
    let pour_k = pour_temp + 273.15;
    let mold_k = 300.0;
    // A guess at the fill time from the inlet speed and a 12 mm choke,
    // so the run lasts long enough to fill and start to freeze.
    let choke_area_m2 = std::f64::consts::PI * 0.006 * 0.006;
    let fill_s = (volume_cm3 * 1e-6 / (choke_area_m2 * pour_speed.max(0.1))).max(1.0);
    let end_s = fill_s * 4.0;
    format!(
        r#"! Truchas input deck written by Anvil. Check every namelist against
! the Truchas reference manual (https://www.truchas.org/docs/) before a
! run; face set ids and the time step depend on the mesh you make from
! cavity.stl (see README.md). Units: SI, temperatures in K.

&MESH
  mesh_file = 'cavity.exo'
/

&OUTPUTS
  output_t  = 0.0, {end_s:.2}
  output_dt = {out_dt:.3}
/

&PHYSICS
  flow = .true.
  heat_transport = .true.
/

&FLOW
  inviscid = .false.
  vol_track_subcycles = 2
/

&NUMERICS
  dt_init = 1.0e-4
  dt_max  = 5.0e-3
  dt_grow = 1.05
/

&MATERIAL
  name = '{tag}'
  phases = '{tag}_solid', '{tag}_liquid'
  density = {rho_l:.0}
/

&PHASE
  name = '{tag}_solid'
  specific_heat = {cp:.0}
  conductivity = {k:.1}
/

&PHASE
  name = '{tag}_liquid'
  specific_heat = {cp:.0}
  conductivity = {k:.1}
  viscosity = 5.0e-3
/

&PHASE_CHANGE
  low_temp_phase  = '{tag}_solid'
  high_temp_phase = '{tag}_liquid'
  solidus_temp  = {solidus:.1}
  liquidus_temp = {liquidus:.1}
  latent_heat   = {latent:.0}
/

! The cavity starts empty (void) at the mold temperature.
&BODY
  surface_name = 'from mesh file'
  mesh_material_number = 1
  material_name = 'VOID'
  temperature = {mold_k:.1}
/

! Face set 1 is the pouring cup rim: metal enters at the pour speed.
&FLOW_BC
  name = 'pour'
  face_set_ids = 1
  type = 'velocity'
  velocity = 0.0, 0.0, -{pour_speed:.2}
  inflow_material = '{tag}'
  inflow_temperature = {pour_k:.1}
/

! Face set 2 is every mold wall: heat leaves by a sand contact
! coefficient. Calibrate htc from one pour of the same sand.
&THERMAL_BC
  name = 'mold'
  face_set_ids = 2
  type = 'htc'
  htc = 500.0
  ambient_temp = {mold_k:.1}
/

&THERMAL_BC
  name = 'pour-inlet'
  face_set_ids = 1
  type = 'temperature'
  temperature = {pour_k:.1}
/
"#,
        end_s = end_s,
        out_dt = (end_s / 40.0).max(0.01),
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

fn truchas_readme(alloy: &Alloy, volume_cm3: f64) -> String {
    format!(
        r#"# Truchas case from Anvil

Cavity volume {volume_cm3:.0} cm3, alloy {name}.

1. Mesh the cavity. Truchas reads Exodus II. One free path:
   `gmsh -3 -clmax 3 cavity.stl -o cavity.msh` then
   `meshio convert cavity.msh cavity.exo` (pip install meshio netCDF4).
   Use a 2 to 3 mm element size for a 3 mm wall.
2. Mark face sets in the mesh: id 1 on the pouring cup rim, id 2 on all
   other faces (the sand contact). Gmsh physical surfaces or a meshio
   script can do this; Truchas needs them for the BCs in casting.inp.
3. Check casting.inp against the Truchas reference manual, then run
   `truchas casting.inp`. Results are Exodus files for ParaView.

The material numbers are starting values from
docs/research/casting_simulation.md. Calibrate the mold heat transfer
coefficient (htc) and the pouring speed from one real pour.
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
        let deck = truchas_deck(&ALLOYS[0], 1400.0, 1.5, 250.0);
        assert!(deck.contains("solidus_temp  = 1423."), "{deck}");
        assert!(deck.contains("latent_heat   = 280000"));
        assert!(deck.contains("inflow_temperature = 1673."));
        let dir = std::env::temp_dir().join("anvil_truchas_test");
        let doc = crate::kettle::kettle_gated();
        let files = write_truchas_case(&doc, 15, &ALLOYS[0], 1400.0, 1.5, &dir).unwrap();
        assert_eq!(files.len(), 3);
        let stl = std::fs::metadata(&files[0]).unwrap().len();
        assert!(stl > 1_000_000, "cavity.stl is {stl} bytes");
    }
}
