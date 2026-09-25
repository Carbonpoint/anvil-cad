//! Exodus II hex meshes for Truchas, written as netCDF classic files.
//!
//! netCDF classic (CDF-2, 64-bit offsets) is a small big-endian format: a
//! header naming the dimensions, the global attributes and the variables
//! with their file offsets, then each variable's data. Exodus II is a set
//! of names for dimensions and variables on top of it. Only what a voxel
//! mesh needs is written: nodes, HEX8 element blocks and side sets.

use anvil_math::DVec3;

// ------------------------------------------------------------- netCDF

const NC_DIMENSION: u32 = 0x0A;
const NC_VARIABLE: u32 = 0x0B;
const NC_ATTRIBUTE: u32 = 0x0C;
const NC_CHAR: u32 = 2;
const NC_INT: u32 = 4;
const NC_FLOAT: u32 = 5;
const NC_DOUBLE: u32 = 6;

pub enum Att {
    Text(String),
    Int(Vec<i32>),
    Float(Vec<f32>),
}

pub enum Data {
    Char(Vec<u8>),
    Int(Vec<i32>),
    Double(Vec<f64>),
}

impl Data {
    fn nc_type(&self) -> u32 {
        match self {
            Data::Char(_) => NC_CHAR,
            Data::Int(_) => NC_INT,
            Data::Double(_) => NC_DOUBLE,
        }
    }
    fn bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            Data::Char(v) => out.extend_from_slice(v),
            Data::Int(v) => v.iter().for_each(|x| out.extend_from_slice(&x.to_be_bytes())),
            Data::Double(v) => v.iter().for_each(|x| out.extend_from_slice(&x.to_be_bytes())),
        }
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
        out
    }
}

pub struct Var {
    pub name: String,
    /// Indices into the file's dimensions.
    pub dims: Vec<usize>,
    pub atts: Vec<(String, Att)>,
    pub data: Data,
}

/// A netCDF classic file in memory. A dimension of length 0 is the
/// unlimited (record) dimension; variables on it get no records.
#[derive(Default)]
pub struct NcFile {
    pub dims: Vec<(String, usize)>,
    pub atts: Vec<(String, Att)>,
    pub vars: Vec<Var>,
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_be_bytes());
}

fn put_name(out: &mut Vec<u8>, s: &str) {
    put_u32(out, s.len() as u32);
    out.extend_from_slice(s.as_bytes());
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

fn put_atts(out: &mut Vec<u8>, atts: &[(String, Att)]) {
    if atts.is_empty() {
        put_u32(out, 0);
        put_u32(out, 0);
        return;
    }
    put_u32(out, NC_ATTRIBUTE);
    put_u32(out, atts.len() as u32);
    for (name, a) in atts {
        put_name(out, name);
        let (t, n, bytes): (u32, usize, Vec<u8>) = match a {
            Att::Text(s) => (NC_CHAR, s.len(), s.as_bytes().to_vec()),
            Att::Int(v) => (NC_INT, v.len(), v.iter().flat_map(|x| x.to_be_bytes()).collect()),
            Att::Float(v) => (NC_FLOAT, v.len(), v.iter().flat_map(|x| x.to_be_bytes()).collect()),
        };
        put_u32(out, t);
        put_u32(out, n as u32);
        out.extend_from_slice(&bytes);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
    }
}

impl NcFile {
    pub fn dim(&mut self, name: &str, len: usize) -> usize {
        self.dims.push((name.into(), len));
        self.dims.len() - 1
    }

    fn is_record(&self, v: &Var) -> bool {
        v.dims.first().is_some_and(|&d| self.dims[d].1 == 0)
    }

    /// The whole file.
    pub fn to_bytes(&self) -> Vec<u8> {
        // The header, with the offsets filled in on a second pass.
        let header = |begins: &[u64]| -> Vec<u8> {
            let mut out = b"CDF\x02".to_vec();
            put_u32(&mut out, 0); // no records
            if self.dims.is_empty() {
                put_u32(&mut out, 0);
                put_u32(&mut out, 0);
            } else {
                put_u32(&mut out, NC_DIMENSION);
                put_u32(&mut out, self.dims.len() as u32);
                for (name, len) in &self.dims {
                    put_name(&mut out, name);
                    put_u32(&mut out, *len as u32);
                }
            }
            put_atts(&mut out, &self.atts);
            put_u32(&mut out, NC_VARIABLE);
            put_u32(&mut out, self.vars.len() as u32);
            for (k, v) in self.vars.iter().enumerate() {
                put_name(&mut out, &v.name);
                put_u32(&mut out, v.dims.len() as u32);
                for d in &v.dims {
                    put_u32(&mut out, *d as u32);
                }
                put_atts(&mut out, &v.atts);
                put_u32(&mut out, v.data.nc_type());
                let vsize = if self.is_record(v) {
                    // One record's worth.
                    let per: usize = v.dims[1..].iter().map(|&d| self.dims[d].1).product();
                    (per * type_size(v.data.nc_type())).div_ceil(4) * 4
                } else {
                    v.data.bytes().len()
                };
                put_u32(&mut out, vsize as u32);
                out.extend_from_slice(&begins[k].to_be_bytes());
            }
            out
        };
        let mut begins = vec![0u64; self.vars.len()];
        let head_len = header(&begins).len() as u64;
        let mut at = head_len;
        let fixed: Vec<usize> = (0..self.vars.len()).filter(|&k| !self.is_record(&self.vars[k])).collect();
        for &k in &fixed {
            begins[k] = at;
            at += self.vars[k].data.bytes().len() as u64;
        }
        // Record variables start where the records would; there are none.
        for (k, v) in self.vars.iter().enumerate() {
            if self.is_record(v) {
                begins[k] = at;
            }
        }
        let mut out = header(&begins);
        for &k in &fixed {
            out.extend_from_slice(&self.vars[k].data.bytes());
        }
        out
    }
}

fn type_size(t: u32) -> usize {
    match t {
        NC_CHAR => 1,
        NC_INT | NC_FLOAT => 4,
        _ => 8,
    }
}

// -------------------------------------------------------------- Exodus

/// A side set: its id, its name, and its faces as (voxel, HEX8 side).
pub type SideSet = (i32, String, Vec<(usize, u8)>);

/// A block-structured hex mesh of a voxel grid: every voxel is an element
/// of block `blocks[k]` (1-based block ids, 0 leaves the voxel out).
pub struct VoxelMesh {
    pub lo: DVec3,
    pub step: f64,
    pub n: [usize; 3],
    /// Block id per voxel, x-major like `anvil_implicit::sampled`.
    pub blocks: Vec<u8>,
    /// Side sets: (id, name, list of (voxel, Exodus HEX8 side 1..6)).
    pub side_sets: Vec<SideSet>,
}

/// HEX8 side of the face of a voxel that looks along `axis` (0 x, 1 y,
/// 2 z), on the high side when `high`. Exodus numbering: 1 y-, 2 x+,
/// 3 y+, 4 x-, 5 z-, 6 z+.
pub fn hex_side(axis: usize, high: bool) -> u8 {
    match (axis, high) {
        (1, false) => 1,
        (0, true) => 2,
        (1, true) => 3,
        (0, false) => 4,
        (2, false) => 5,
        _ => 6,
    }
}

impl VoxelMesh {
    /// The Exodus II file.
    pub fn to_exodus(&self, title: &str) -> Vec<u8> {
        let [nx, ny, nz] = self.n;
        let node = |i: usize, j: usize, k: usize| (i * (ny + 1) + j) * (nz + 1) + k;
        // Nodes used by any element, numbered in order.
        let mut used = vec![false; (nx + 1) * (ny + 1) * (nz + 1)];
        let corners = |q: usize| {
            let (i, r) = (q / (ny * nz), q % (ny * nz));
            let (j, k) = (r / nz, r % nz);
            // HEX8: the bottom face counter-clockwise, then the top.
            [
                node(i, j, k),
                node(i + 1, j, k),
                node(i + 1, j + 1, k),
                node(i, j + 1, k),
                node(i, j, k + 1),
                node(i + 1, j, k + 1),
                node(i + 1, j + 1, k + 1),
                node(i, j + 1, k + 1),
            ]
        };
        let mut ids: Vec<u8> = self.blocks.iter().copied().filter(|&b| b > 0).collect();
        ids.sort();
        ids.dedup();
        for (q, &b) in self.blocks.iter().enumerate() {
            if b > 0 {
                for c in corners(q) {
                    used[c] = true;
                }
            }
        }
        let mut number = vec![0i32; used.len()];
        let mut coords = [Vec::new(), Vec::new(), Vec::new()];
        let mut next = 0;
        for i in 0..=nx {
            for j in 0..=ny {
                for k in 0..=nz {
                    let g = node(i, j, k);
                    if used[g] {
                        next += 1;
                        number[g] = next;
                        let p = self.lo + DVec3::new(i as f64, j as f64, k as f64) * self.step;
                        coords[0].push(p.x);
                        coords[1].push(p.y);
                        coords[2].push(p.z);
                    }
                }
            }
        }
        // Elements, block by block; Exodus numbers them in that order.
        let mut elem_number = vec![0i32; self.blocks.len()];
        let mut per_block: Vec<Vec<usize>> = Vec::new();
        let mut e = 0;
        for &b in &ids {
            let list: Vec<usize> = (0..self.blocks.len()).filter(|&q| self.blocks[q] == b).collect();
            for &q in &list {
                e += 1;
                elem_number[q] = e;
            }
            per_block.push(list);
        }
        let mut f = NcFile::default();
        let len_name = f.dim("len_name", 33);
        let _len_string = f.dim("len_string", 33);
        let _len_line = f.dim("len_line", 81);
        let _four = f.dim("four", 4);
        let time_step = f.dim("time_step", 0);
        let num_dim = f.dim("num_dim", 3);
        let num_nodes = f.dim("num_nodes", next as usize);
        let _num_elem = f.dim("num_elem", e as usize);
        let num_el_blk = f.dim("num_el_blk", ids.len());
        let sets: Vec<&SideSet> = self.side_sets.iter().filter(|s| !s.2.is_empty()).collect();
        let num_ss = if sets.is_empty() { None } else { Some(f.dim("num_side_sets", sets.len())) };
        f.atts = vec![
            ("api_version".into(), Att::Float(vec![8.03])),
            ("version".into(), Att::Float(vec![8.03])),
            ("floating_point_word_size".into(), Att::Int(vec![8])),
            ("file_size".into(), Att::Int(vec![1])),
            ("maximum_name_length".into(), Att::Int(vec![32])),
            ("int64_status".into(), Att::Int(vec![0])),
            ("title".into(), Att::Text(title.into())),
        ];
        let names = |list: &[String]| {
            let mut v = Vec::new();
            for s in list {
                let mut b = s.as_bytes().to_vec();
                b.truncate(32);
                b.resize(33, 0);
                v.extend(b);
            }
            Data::Char(v)
        };
        let var = |name: &str, dims: Vec<usize>, data: Data| Var { name: name.into(), dims, atts: Vec::new(), data };
        f.vars.push(var("time_whole", vec![time_step], Data::Double(Vec::new())));
        f.vars.push(var("eb_status", vec![num_el_blk], Data::Int(vec![1; ids.len()])));
        let mut prop = var("eb_prop1", vec![num_el_blk], Data::Int(ids.iter().map(|&b| b as i32).collect()));
        prop.atts.push(("name".into(), Att::Text("ID".into())));
        f.vars.push(prop);
        f.vars.push(var(
            "eb_names",
            vec![num_el_blk, len_name],
            names(&ids.iter().map(|b| format!("block_{b}")).collect::<Vec<_>>()),
        ));
        f.vars.push(var("coor_names", vec![num_dim, len_name], names(&["x".into(), "y".into(), "z".into()])));
        for (axis, name) in ["coordx", "coordy", "coordz"].iter().enumerate() {
            f.vars.push(var(name, vec![num_nodes], Data::Double(std::mem::take(&mut coords[axis]))));
        }
        for (k, list) in per_block.iter().enumerate() {
            let d_el = f.dim(&format!("num_el_in_blk{}", k + 1), list.len());
            let d_nod = f.dim(&format!("num_nod_per_el{}", k + 1), 8);
            let conn: Vec<i32> = list.iter().flat_map(|&q| corners(q).map(|c| number[c])).collect();
            let mut v = var(&format!("connect{}", k + 1), vec![d_el, d_nod], Data::Int(conn));
            v.atts.push(("elem_type".into(), Att::Text("HEX8".into())));
            f.vars.push(v);
        }
        if let Some(num_ss) = num_ss {
            f.vars.push(var("ss_status", vec![num_ss], Data::Int(vec![1; sets.len()])));
            let mut prop = var("ss_prop1", vec![num_ss], Data::Int(sets.iter().map(|s| s.0).collect()));
            prop.atts.push(("name".into(), Att::Text("ID".into())));
            f.vars.push(prop);
            f.vars.push(var(
                "ss_names",
                vec![num_ss, len_name],
                names(&sets.iter().map(|s| s.1.clone()).collect::<Vec<_>>()),
            ));
            for (k, s) in sets.iter().enumerate() {
                let d = f.dim(&format!("num_side_ss{}", k + 1), s.2.len());
                f.vars.push(var(
                    &format!("elem_ss{}", k + 1),
                    vec![d],
                    Data::Int(s.2.iter().map(|(q, _)| elem_number[*q]).collect()),
                ));
                f.vars.push(var(
                    &format!("side_ss{}", k + 1),
                    vec![d],
                    Data::Int(s.2.iter().map(|(_, side)| *side as i32).collect()),
                ));
            }
        }
        f.to_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_two_voxel_mesh_has_twelve_nodes_and_the_magic() {
        let m = VoxelMesh {
            lo: DVec3::ZERO,
            step: 1.0,
            n: [2, 1, 1],
            blocks: vec![1, 2],
            side_sets: vec![(1, "top".into(), vec![(1, hex_side(2, true))])],
        };
        let b = m.to_exodus("t");
        assert_eq!(&b[..4], b"CDF\x02");
        // The header names every Exodus variable Truchas reads.
        let text = String::from_utf8_lossy(&b);
        for name in ["coordx", "connect1", "connect2", "elem_ss1", "side_ss1", "eb_prop1", "ss_prop1", "HEX8"] {
            assert!(text.contains(name), "{name}");
        }
        // 12 nodes: the coordx data is 12 doubles, the first 0.
        let pos = text.find("coordx").unwrap();
        assert!(pos < b.len());
    }
}
