//! External aerodynamics case export: the visible bodies as one STL in
//! metres plus a minimal OpenFOAM case (blockMesh box, snappyHexMesh
//! around the body, simpleFoam with k omega SST, force coefficients),
//! so a design loop can hand each variant to a CFD run. The case is a
//! template written from the standard tutorials; it has not been run
//! here. See docs/research/wing_aero_loop.md.

use crate::IoError;
use anvil_feature::Document;
use std::path::{Path, PathBuf};

/// Reference values for the force coefficients.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AeroCase {
    /// Free stream speed, m/s.
    pub speed: f64,
    /// Angle of attack, degrees, about the Y axis (nose up positive).
    pub alpha_deg: f64,
    /// Reference area, m2.
    pub area_m2: f64,
    /// Reference chord, m.
    pub chord_m: f64,
}

fn write(dir: &Path, rel: &str, text: &str, out: &mut Vec<PathBuf>) -> Result<(), IoError> {
    let p = dir.join(rel);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&p, text)?;
    out.push(p);
    Ok(())
}

fn header(class: &str, object: &str) -> String {
    format!(
        "FoamFile\n{{\n    version     2.0;\n    format      ascii;\n    class       {class};\n    object      {object};\n}}\n\n"
    )
}

/// Write the case into `dir`. Returns the files written.
pub fn write_openfoam_case(doc: &Document, case: &AeroCase, dir: &Path) -> Result<Vec<PathBuf>, IoError> {
    use anvil_feature::features::aircraft::tag_name;
    // Triangles grouped by the component tag on their face (1 to 4 on an
    // aircraft body; everything else is "body"), in metres.
    let mut groups: std::collections::BTreeMap<String, Vec<[anvil_math::DVec3; 3]>> = std::collections::BTreeMap::new();
    let mut lo = anvil_math::DVec3::splat(f64::INFINITY);
    let mut hi = anvil_math::DVec3::splat(f64::NEG_INFINITY);
    let mut count = 0usize;
    for (_, body) in doc.visible_bodies() {
        let m = anvil_kernel::mesh::tessellate(body);
        for (k, t) in m.indices.as_chunks::<3>().0.iter().enumerate() {
            let tri = [
                m.positions[t[0] as usize] * 0.001,
                m.positions[t[1] as usize] * 0.001,
                m.positions[t[2] as usize] * 0.001,
            ];
            for p in &tri {
                lo = lo.min(*p);
                hi = hi.max(*p);
            }
            let name = m
                .face_of_tri
                .get(k)
                .and_then(|fid| body.faces.get(*fid))
                .and_then(|f| match f.surface {
                    anvil_kernel::Surface::Revolved { id } if (1..=4).contains(&id) => Some(tag_name(id)),
                    _ => None,
                })
                .unwrap_or("body");
            groups.entry(name.to_string()).or_default().push(tri);
            count += 1;
        }
    }
    if count == 0 {
        lo = anvil_math::DVec3::ZERO;
        hi = anvil_math::DVec3::ONE;
    }
    let mut files = Vec::new();
    std::fs::create_dir_all(dir.join("constant/triSurface"))?;
    let stl = dir.join("constant/triSurface/aircraft.stl");
    {
        // ASCII STL with one named solid per component: snappyHexMesh
        // turns each solid into its own patch.
        let mut text = String::new();
        for (name, tris) in &groups {
            text.push_str(&format!("solid {name}\n"));
            for t in tris {
                let n = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
                text.push_str(&format!("  facet normal {} {} {}\n    outer loop\n", n.x, n.y, n.z));
                for p in t {
                    text.push_str(&format!("      vertex {} {} {}\n", p.x, p.y, p.z));
                }
                text.push_str("    endloop\n  endfacet\n");
            }
            text.push_str(&format!("endsolid {name}\n"));
        }
        std::fs::write(&stl, text)?;
    }
    files.push(stl);
    let region_names: Vec<String> = groups.keys().cloned().collect();
    let regions: String = region_names.iter().map(|n| format!("            {n} {{ name {n}; }}\n")).collect();
    let refine_regions: String = region_names
        .iter()
        .map(|n| format!("                {n} {{ level (5 6); patchInfo {{ type wall; }} }}\n"))
        .collect();
    let patches: String = region_names.iter().map(|n| format!("aircraft_{n}")).collect::<Vec<_>>().join(" ");

    let len = (hi.x - lo.x).max(1e-3);
    let span = (hi.y - lo.y).max(1e-3);
    let height = (hi.z - lo.z).max(1e-3);
    let cx = 0.5 * (lo.x + hi.x);
    let cy = 0.5 * (lo.y + hi.y);
    let cz = 0.5 * (lo.z + hi.z);
    // Domain: 5 lengths ahead, 10 behind, 4 spans to each side and up and down.
    let (x0, x1) = (cx - 5.5 * len, cx + 10.5 * len);
    let (y0, y1) = (cy - 4.0 * span, cy + 4.0 * span);
    let (z0, z1) = (cz - 4.0 * span.max(height), cz + 4.0 * span.max(height));
    let alpha = case.alpha_deg.to_radians();
    let (ux, uz) = (case.speed * alpha.cos(), case.speed * alpha.sin());
    let nu = 1.46e-5;
    let turb_i = 0.01;
    let k = 1.5 * (case.speed * turb_i).powi(2);
    let omega = k.sqrt() / (0.09f64.powf(0.25) * 0.1 * case.chord_m.max(1e-3));

    write(
        dir,
        "system/blockMeshDict",
        &format!(
            "{}scale 1;\n\nvertices\n(\n    ({x0} {y0} {z0})\n    ({x1} {y0} {z0})\n    ({x1} {y1} {z0})\n    ({x0} {y1} {z0})\n    ({x0} {y0} {z1})\n    ({x1} {y0} {z1})\n    ({x1} {y1} {z1})\n    ({x0} {y1} {z1})\n);\n\nblocks\n(\n    hex (0 1 2 3 4 5 6 7) (48 32 32) simpleGrading (1 1 1)\n);\n\nedges\n(\n);\n\nboundary\n(\n    inlet\n    {{\n        type patch;\n        faces ((0 4 7 3));\n    }}\n    outlet\n    {{\n        type patch;\n        faces ((1 2 6 5));\n    }}\n    sides\n    {{\n        type patch;\n        faces ((0 1 5 4) (3 7 6 2) (0 3 2 1) (4 5 6 7));\n    }}\n);\n\nmergePatchPairs\n(\n);\n",
            header("dictionary", "blockMeshDict")
        ),
        &mut files,
    )?;
    write(
        dir,
        "system/snappyHexMeshDict",
        &format!(
            "{}castellatedMesh true;\nsnap            true;\naddLayers       false;\n\ngeometry\n{{\n    aircraft.stl\n    {{\n        type triSurfaceMesh;\n        name aircraft;\n        regions\n        {{\n{regions}        }}\n    }}\n    refineBox\n    {{\n        type searchableBox;\n        min ({} {} {});\n        max ({} {} {});\n    }}\n}}\n\ncastellatedMeshControls\n{{\n    maxLocalCells 2000000;\n    maxGlobalCells 8000000;\n    minRefinementCells 10;\n    maxLoadUnbalance 0.10;\n    nCellsBetweenLevels 3;\n    features ();\n    refinementSurfaces\n    {{\n        aircraft\n        {{\n            level (5 6);\n            patchInfo {{ type wall; }}\n            regions\n            {{\n{refine_regions}            }}\n        }}\n    }}\n    resolveFeatureAngle 30;\n    refinementRegions\n    {{\n        refineBox {{ mode inside; levels ((1E15 3)); }}\n    }}\n    locationInMesh ({} {} {});\n    allowFreeStandingZoneFaces true;\n}}\n\nsnapControls\n{{\n    nSmoothPatch 3;\n    tolerance 2.0;\n    nSolveIter 30;\n    nRelaxIter 5;\n    nFeatureSnapIter 10;\n    implicitFeatureSnap true;\n    explicitFeatureSnap false;\n    multiRegionFeatureSnap false;\n}}\n\naddLayersControls\n{{\n    relativeSizes true;\n    layers {{}}\n    expansionRatio 1.2;\n    finalLayerThickness 0.3;\n    minThickness 0.1;\n    nGrow 0;\n    featureAngle 60;\n    nRelaxIter 3;\n    nSmoothSurfaceNormals 1;\n    nSmoothNormals 3;\n    nSmoothThickness 10;\n    maxFaceThicknessRatio 0.5;\n    maxThicknessToMedialRatio 0.3;\n    minMedianAxisAngle 90;\n    nBufferCellsNoExtrude 0;\n    nLayerIter 50;\n}}\n\nmeshQualityControls\n{{\n    maxNonOrtho 65;\n    maxBoundarySkewness 20;\n    maxInternalSkewness 4;\n    maxConcave 80;\n    minVol 1e-13;\n    minTetQuality 1e-15;\n    minArea -1;\n    minTwist 0.02;\n    minDeterminant 0.001;\n    minFaceWeight 0.02;\n    minVolRatio 0.01;\n    minTriangleTwist -1;\n    nSmoothScale 4;\n    errorReduction 0.75;\n}}\n\nmergeTolerance 1e-6;\n",
            header("dictionary", "snappyHexMeshDict"),
            lo.x - len,
            lo.y - 0.3 * span,
            lo.z - height,
            hi.x + 3.0 * len,
            hi.y + 0.3 * span,
            hi.z + height,
            x0 + 0.5 * len,
            y0 + 0.5 * span,
            z1 - 0.5 * height
        ),
        &mut files,
    )?;
    write(
        dir,
        "system/controlDict",
        &format!(
            "{}application     simpleFoam;\nstartFrom       startTime;\nstartTime       0;\nstopAt          endTime;\nendTime         600;\ndeltaT          1;\nwriteControl    timeStep;\nwriteInterval   100;\npurgeWrite      2;\nwriteFormat     ascii;\nwritePrecision  8;\nwriteCompression off;\ntimeFormat      general;\ntimePrecision   6;\nrunTimeModifiable true;\n\nfunctions\n{{\n    forceCoeffs\n    {{\n        type forceCoeffs;\n        libs (forces);\n        writeControl timeStep;\n        writeInterval 1;\n        patches ({patches});\n        rho rhoInf;\n        rhoInf 1.225;\n        CofR ({cx} {cy} {cz});\n        liftDir ({} 0 {});\n        dragDir ({} 0 {});\n        pitchAxis (0 1 0);\n        magUInf {};\n        lRef {};\n        Aref {};\n    }}\n}}\n",
            header("dictionary", "controlDict"),
            -alpha.sin(),
            alpha.cos(),
            alpha.cos(),
            alpha.sin(),
            case.speed,
            case.chord_m,
            case.area_m2
        ),
        &mut files,
    )?;
    write(
        dir,
        "system/fvSchemes",
        &format!(
            "{}ddtSchemes {{ default steadyState; }}\ngradSchemes {{ default Gauss linear; limited cellLimited Gauss linear 1; grad(U) $limited; grad(k) $limited; grad(omega) $limited; }}\ndivSchemes\n{{\n    default none;\n    div(phi,U) bounded Gauss linearUpwindV grad(U);\n    div(phi,k) bounded Gauss limitedLinear 1;\n    div(phi,omega) bounded Gauss limitedLinear 1;\n    div((nuEff*dev2(T(grad(U))))) Gauss linear;\n}}\nlaplacianSchemes {{ default Gauss linear corrected; }}\ninterpolationSchemes {{ default linear; }}\nsnGradSchemes {{ default corrected; }}\nwallDist {{ method meshWave; }}\n",
            header("dictionary", "fvSchemes")
        ),
        &mut files,
    )?;
    write(
        dir,
        "system/fvSolution",
        &format!(
            "{}solvers\n{{\n    p {{ solver GAMG; smoother GaussSeidel; tolerance 1e-7; relTol 0.01; }}\n    \"(U|k|omega)\" {{ solver smoothSolver; smoother GaussSeidel; tolerance 1e-8; relTol 0.1; nSweeps 2; }}\n}}\nSIMPLE\n{{\n    nNonOrthogonalCorrectors 0;\n    consistent yes;\n    residualControl {{ p 1e-5; U 1e-5; \"(k|omega)\" 1e-5; }}\n}}\nrelaxationFactors\n{{\n    equations {{ U 0.9; \".*\" 0.9; }}\n}}\n",
            header("dictionary", "fvSolution")
        ),
        &mut files,
    )?;
    write(
        dir,
        "constant/transportProperties",
        &format!("{}transportModel Newtonian;\nnu {nu};\n", header("dictionary", "transportProperties")),
        &mut files,
    )?;
    write(
        dir,
        "constant/turbulenceProperties",
        &format!(
            "{}simulationType RAS;\nRAS\n{{\n    RASModel kOmegaSST;\n    turbulence on;\n    printCoeffs on;\n}}\n",
            header("dictionary", "turbulenceProperties")
        ),
        &mut files,
    )?;
    let field = |class: &str, object: &str, dims: &str, internal: &str, inlet: &str, wall: &str| {
        format!(
            "{}dimensions {dims};\ninternalField {internal};\nboundaryField\n{{\n    inlet {{ {inlet} }}\n    outlet {{ type zeroGradient; }}\n    sides {{ type slip; }}\n    \"aircraft.*\" {{ {wall} }}\n}}\n",
            header(class, object)
        )
    };
    write(
        dir,
        "0/U",
        &field(
            "volVectorField",
            "U",
            "[0 1 -1 0 0 0 0]",
            &format!("uniform ({ux} 0 {uz})"),
            &format!("type fixedValue; value uniform ({ux} 0 {uz});"),
            "type noSlip;",
        ),
        &mut files,
    )?;
    write(
        dir,
        "0/p",
        &field("volScalarField", "p", "[0 2 -2 0 0 0 0]", "uniform 0", "type zeroGradient;", "type zeroGradient;"),
        &mut files,
    )?;
    let mut p_text = std::fs::read_to_string(dir.join("0/p"))?;
    p_text = p_text.replace("outlet { type zeroGradient; }", "outlet { type fixedValue; value uniform 0; }");
    std::fs::write(dir.join("0/p"), p_text)?;
    write(
        dir,
        "0/k",
        &field(
            "volScalarField",
            "k",
            "[0 2 -2 0 0 0 0]",
            &format!("uniform {k}"),
            &format!("type fixedValue; value uniform {k};"),
            "type kqRWallFunction; value uniform 1e-10;",
        ),
        &mut files,
    )?;
    write(
        dir,
        "0/omega",
        &field(
            "volScalarField",
            "omega",
            "[0 0 -1 0 0 0 0]",
            &format!("uniform {omega}"),
            &format!("type fixedValue; value uniform {omega};"),
            "type omegaWallFunction; value uniform 1;",
        ),
        &mut files,
    )?;
    write(
        dir,
        "0/nut",
        &field(
            "volScalarField",
            "nut",
            "[0 2 -1 0 0 0 0]",
            "uniform 0",
            "type calculated; value uniform 0;",
            "type nutkWallFunction; value uniform 0;",
        ),
        &mut files,
    )?;
    write(
        dir,
        "Allrun",
        "#!/bin/sh\ncd \"$(dirname \"$0\")\" || exit 1\n. \"$WM_PROJECT_DIR/bin/tools/RunFunctions\"\nrunApplication blockMesh\nrunApplication surfaceFeatureExtract || true\nrunApplication snappyHexMesh -overwrite\nrunApplication simpleFoam\n",
        &mut files,
    )?;
    write(
        dir,
        "README.md",
        &format!(
            "# OpenFOAM case from Anvil\n\nBody: `constant/triSurface/aircraft.stl`, in metres, {} triangles, one\nnamed solid per component ({}), so snappyHexMesh makes a patch per\ncomponent and the force coefficients can be split by patch.\nbounds {:.3} to {:.3} m in x, {:.3} to {:.3} m in y, {:.3} to {:.3} m in z.\nFree stream {} m/s at {} degrees; reference area {} m2, chord {} m.\n\nRun with OpenFOAM v2312 or later:\n\n    ./Allrun\n\n`postProcessing/forceCoeffs/0/coefficient.dat` then holds Cl and Cd per\niteration; take the last converged line. The mesh has no boundary\nlayers (`addLayers false`) and a coarse box, so the numbers are a first\nlook; add layers and refine before trusting drag. This case was written\nfrom the standard tutorials and has not been run by Anvil.\n",
            count,
            region_names.join(", "),
            lo.x,
            hi.x,
            lo.y,
            hi.y,
            lo.z,
            hi.z,
            case.speed,
            case.alpha_deg,
            case.area_m2,
            case.chord_m
        ),
        &mut files,
    )?;
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_feature::features::primitives::BoxFeature;

    #[test]
    fn case_has_every_file_and_the_inlet_speed() {
        let mut d = Document::new("t");
        d.add_feature(Box::new(BoxFeature {
            x: "0".into(),
            y: "-50".into(),
            z: "0".into(),
            width: "100".into(),
            depth: "100".into(),
            height: "10".into(),
        }));
        let dir = std::env::temp_dir().join(format!("anvil_foam_{}", std::process::id()));
        let files =
            write_openfoam_case(&d, &AeroCase { speed: 20.0, alpha_deg: 4.0, area_m2: 0.01, chord_m: 0.1 }, &dir)
                .unwrap();
        assert!(files.len() >= 13, "{} files", files.len());
        let u = std::fs::read_to_string(dir.join("0/U")).unwrap();
        assert!(u.contains("19.95") && u.contains("1.395"), "{u}");
        let ctl = std::fs::read_to_string(dir.join("system/controlDict")).unwrap();
        assert!(ctl.contains("forceCoeffs") && ctl.contains("Aref 0.01"));
        let stl = std::fs::read_to_string(dir.join("constant/triSurface/aircraft.stl")).unwrap();
        assert!(stl.starts_with("solid body") && stl.matches("facet normal").count() == 12, "{}", &stl[..60]);
        let snappy = std::fs::read_to_string(dir.join("system/snappyHexMeshDict")).unwrap();
        assert!(snappy.contains("body { name body; }"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
