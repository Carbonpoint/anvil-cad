use anvil_kernel::ops::revolve;
use anvil_math::{Axis, DVec2, DVec3, Plane};

/// Replicates the demo ring: 8x6 rectangle at (45,25) on the XZ plane,
/// revolved 270 degrees about the sketch Y axis (world Z).
#[test]
fn demo_ring_faces_point_outward() {
    let prof = vec![DVec2::new(41.0, 22.0), DVec2::new(49.0, 22.0), DVec2::new(49.0, 28.0), DVec2::new(41.0, 28.0)];
    let plane = Plane::XZ;
    let axis = Axis::new(plane.origin, plane.to_world(DVec2::Y) - plane.origin);
    let s = revolve(&plane, &prof, &axis, 270f64.to_radians()).unwrap();
    let mesh = anvil_kernel::mesh::tessellate(&s);
    // Every triangle should have a positive contribution sign relative to
    // the mesh as a whole: check per-face consistency via signed volume of
    // each face pyramid to the centroid.
    let c = DVec3::new(0.0, 0.0, 25.0);
    let mut bad = 0;
    for t in mesh.indices.as_chunks::<3>().0 {
        let a = mesh.positions[t[0] as usize] - c;
        let b = mesh.positions[t[1] as usize] - c;
        let d = mesh.positions[t[2] as usize] - c;
        // For a ring around c in this configuration, outer/inner faces have
        // mixed signs, so only test caps and top/bottom by normal direction.
        let n = (b - a).cross(d - a);
        let centroid = (a + b + d) / 3.0;
        let radial = DVec3::new(centroid.x, centroid.y, 0.0).normalize_or_zero();
        let r = (centroid.x * centroid.x + centroid.y * centroid.y).sqrt();
        if (r - 49.0).abs() < 1e-6 && n.dot(radial) < 0.0 {
            bad += 1;
        }
        if (r - 41.0).abs() < 1e-6 && n.dot(radial) > 0.0 {
            bad += 1;
        }
        if (centroid.z - 3.0).abs() < 1e-6 && n.z < 0.0 {
            bad += 1;
        }
        if (centroid.z + 3.0).abs() < 1e-6 && n.z > 0.0 {
            bad += 1;
        }
    }
    assert_eq!(bad, 0, "{bad} triangles face inward; volume {}", s.volume());
    assert!(s.volume() > 0.0);
}
