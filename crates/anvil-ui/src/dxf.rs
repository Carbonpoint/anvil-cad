//! Minimal DXF reader for sketches: LINE, CIRCLE, ARC, LWPOLYLINE.
//!
//! DXF is a list of (group code, value) line pairs. Entities start with
//! group 0. Coordinates use codes 10/20 (and 11/21 for a line end), radius
//! 40, arc angles 50/51 in degrees. LWPOLYLINE bulges are drawn as straight
//! segments; flag 70 bit 1 closes the polyline.

use anvil_math::DVec2;
use anvil_sketch::Sketch;

pub fn import(text: &str, sk: &mut Sketch) -> usize {
    let lines: Vec<&str> = text.lines().map(|l| l.trim()).collect();
    let mut pairs: Vec<(i32, &str)> = Vec::new();
    let mut i = 0;
    while i + 1 < lines.len() {
        if let Ok(code) = lines[i].parse::<i32>() {
            pairs.push((code, lines[i + 1]));
        }
        i += 2;
    }
    let mut count = 0;
    let mut k = 0;
    while k < pairs.len() {
        if pairs[k].0 != 0 {
            k += 1;
            continue;
        }
        let kind = pairs[k].1;
        let start = k + 1;
        let mut end = start;
        while end < pairs.len() && pairs[end].0 != 0 {
            end += 1;
        }
        let body = &pairs[start..end];
        let num = |code: i32| body.iter().find(|(c, _)| *c == code).and_then(|(_, v)| v.parse::<f64>().ok());
        match kind {
            "LINE" => {
                if let (Some(x1), Some(y1), Some(x2), Some(y2)) = (num(10), num(20), num(11), num(21)) {
                    let a = sk.add_point(x1, y1);
                    let b = sk.add_point(x2, y2);
                    sk.add_line(a, b);
                    count += 1;
                }
            }
            "CIRCLE" => {
                if let (Some(x), Some(y), Some(r)) = (num(10), num(20), num(40)) {
                    let c = sk.add_point(x, y);
                    sk.add_circle(c, r);
                    count += 1;
                }
            }
            "ARC" => {
                if let (Some(x), Some(y), Some(r), Some(a0), Some(a1)) = (num(10), num(20), num(40), num(50), num(51)) {
                    let c = DVec2::new(x, y);
                    let p = |deg: f64| c + DVec2::new(deg.to_radians().cos(), deg.to_radians().sin()) * r;
                    sk.add_arc_center(c, p(a0), p(a1));
                    count += 1;
                }
            }
            "LWPOLYLINE" => {
                let xs: Vec<f64> = body.iter().filter(|(c, _)| *c == 10).filter_map(|(_, v)| v.parse().ok()).collect();
                let ys: Vec<f64> = body.iter().filter(|(c, _)| *c == 20).filter_map(|(_, v)| v.parse().ok()).collect();
                let closed = num(70).map(|f| (f as i64) & 1 == 1).unwrap_or(false);
                let ids: Vec<_> = xs.iter().zip(ys.iter()).map(|(x, y)| sk.add_point(*x, *y)).collect();
                for w in ids.windows(2) {
                    sk.add_line(w[0], w[1]);
                }
                if closed && ids.len() > 2 {
                    sk.add_line(*ids.last().unwrap(), ids[0]);
                }
                if ids.len() > 1 {
                    count += 1;
                }
            }
            _ => {}
        }
        k = end;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_math::Plane;

    #[test]
    fn reads_polyline_and_circle() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nLWPOLYLINE\n90\n4\n70\n1\n10\n0\n20\n0\n10\n10\n20\n0\n10\n10\n20\n10\n10\n0\n20\n10\n0\nCIRCLE\n10\n5\n20\n5\n40\n2\n0\nENDSEC\n0\nEOF\n";
        let mut sk = Sketch::new(Plane::XY);
        assert_eq!(import(dxf, &mut sk), 2);
        assert_eq!(sk.profiles().len(), 2);
    }
}
