//! Minimal STL reader used by the Insert Mesh feature. `anvil-io` has the
//! full reader; this copy keeps `anvil-feature` free of that dependency.

use anvil_math::DVec3;

pub fn load(path: &str) -> Result<Vec<[DVec3; 3]>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let binary_len = if bytes.len() >= 84 {
        Some(84 + u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize * 50)
    } else {
        None
    };
    let mut tris = Vec::new();
    if binary_len == Some(bytes.len()) {
        let count = (bytes.len() - 84) / 50;
        let f = |o: usize| f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]) as f64;
        for i in 0..count {
            let base = 84 + i * 50 + 12;
            let p = |k: usize| DVec3::new(f(base + k * 12), f(base + k * 12 + 4), f(base + k * 12 + 8));
            tris.push([p(0), p(1), p(2)]);
        }
    } else {
        let text = String::from_utf8_lossy(&bytes);
        let mut cur: Vec<DVec3> = Vec::new();
        for line in text.lines() {
            let mut it = line.split_whitespace();
            if it.next() == Some("vertex") {
                let v: Vec<f64> = it.take(3).filter_map(|x| x.parse().ok()).collect();
                if v.len() == 3 {
                    cur.push(DVec3::new(v[0], v[1], v[2]));
                }
                if cur.len() == 3 {
                    tris.push([cur[0], cur[1], cur[2]]);
                    cur.clear();
                }
            }
        }
    }
    Ok(tris)
}
