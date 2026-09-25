//! A small zip reader: enough to open a 3MF package. It reads stored and
//! deflated entries through the central directory. Zip64 and encrypted
//! entries are refused with an error.

use std::io::Read;

/// Largest entry this reader will inflate, in bytes. A 3MF model of a
/// few million triangles stays far below it.
const MAX_ENTRY: u64 = 1 << 30;

pub struct Zip<'a> {
    data: &'a [u8],
    entries: Vec<Entry>,
}

struct Entry {
    name: String,
    method: u16,
    flags: u16,
    compressed: u64,
    size: u64,
    local: u64,
}

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*d.get(o)?, *d.get(o + 1)?]))
}

fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes([*d.get(o)?, *d.get(o + 1)?, *d.get(o + 2)?, *d.get(o + 3)?]))
}

impl<'a> Zip<'a> {
    pub fn open(data: &'a [u8]) -> Result<Self, String> {
        let bad = || "not a zip file, or a damaged one".to_string();
        // The end of central directory record is in the last 64 kB.
        let lo = data.len().saturating_sub(22 + 65535);
        let eocd = (lo..data.len().saturating_sub(21))
            .rev()
            .find(|&i| u32_at(data, i) == Some(0x0605_4b50))
            .ok_or_else(bad)?;
        let count = u16_at(data, eocd + 10).ok_or_else(bad)? as usize;
        let cd = u32_at(data, eocd + 16).ok_or_else(bad)? as usize;
        if count == 0xFFFF || cd == 0xFFFF_FFFF {
            return Err("zip64 packages are not supported".into());
        }
        let mut entries = Vec::with_capacity(count.min(4096));
        let mut o = cd;
        for _ in 0..count {
            if u32_at(data, o) != Some(0x0201_4b50) {
                return Err(bad());
            }
            let flags = u16_at(data, o + 8).ok_or_else(bad)?;
            let method = u16_at(data, o + 10).ok_or_else(bad)?;
            let compressed = u32_at(data, o + 20).ok_or_else(bad)? as u64;
            let size = u32_at(data, o + 24).ok_or_else(bad)? as u64;
            let n = u16_at(data, o + 28).ok_or_else(bad)? as usize;
            let e = u16_at(data, o + 30).ok_or_else(bad)? as usize;
            let c = u16_at(data, o + 32).ok_or_else(bad)? as usize;
            let local = u32_at(data, o + 42).ok_or_else(bad)? as u64;
            let name = data.get(o + 46..o + 46 + n).ok_or_else(bad)?;
            entries.push(Entry {
                name: String::from_utf8_lossy(name).into_owned(),
                method,
                flags,
                compressed,
                size,
                local,
            });
            o += 46 + n + e + c;
        }
        Ok(Zip { data, entries })
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|e| e.name.as_str())
    }

    /// The bytes of one entry, by name. Names compare without case, and a
    /// leading '/' is ignored.
    pub fn read(&self, name: &str) -> Result<Vec<u8>, String> {
        let want = name.trim_start_matches('/');
        let e = self
            .entries
            .iter()
            .find(|e| e.name.trim_start_matches('/').eq_ignore_ascii_case(want))
            .ok_or_else(|| format!("the package has no {name}"))?;
        if e.flags & 1 != 0 {
            return Err(format!("{name} is encrypted"));
        }
        if e.size > MAX_ENTRY {
            return Err(format!("{name} is too large"));
        }
        let bad = || format!("{name}: damaged zip entry");
        let lo = e.local as usize;
        if u32_at(self.data, lo) != Some(0x0403_4b50) {
            return Err(bad());
        }
        let n = u16_at(self.data, lo + 26).ok_or_else(bad)? as usize;
        let x = u16_at(self.data, lo + 28).ok_or_else(bad)? as usize;
        let start = lo + 30 + n + x;
        let raw = self.data.get(start..start + e.compressed as usize).ok_or_else(bad)?;
        match e.method {
            0 => Ok(raw.to_vec()),
            8 => {
                let mut out = Vec::with_capacity(e.size.min(MAX_ENTRY) as usize);
                flate2::read::DeflateDecoder::new(raw)
                    .take(MAX_ENTRY + 1)
                    .read_to_end(&mut out)
                    .map_err(|err| format!("{name}: {err}"))?;
                if out.len() as u64 > MAX_ENTRY {
                    return Err(format!("{name} is too large"));
                }
                Ok(out)
            }
            m => Err(format!("{name} uses zip method {m}, which is not supported")),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A zip of `(name, data, uncompressed size, method)` entries. The CRC
    /// is left at zero: the reader does not check it.
    pub fn build(files: &[(&str, &[u8], u32, u16)]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, data, size, method) in files {
            let off = out.len() as u32;
            out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
            out.extend_from_slice(&[20, 0, 0, 0]);
            out.extend_from_slice(&method.to_le_bytes());
            out.extend_from_slice(&[0; 8]);
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(data);
            central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            central.extend_from_slice(&[20, 0, 20, 0, 0, 0]);
            central.extend_from_slice(&method.to_le_bytes());
            central.extend_from_slice(&[0; 8]);
            central.extend_from_slice(&(data.len() as u32).to_le_bytes());
            central.extend_from_slice(&size.to_le_bytes());
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            central.extend_from_slice(&[0; 12]);
            central.extend_from_slice(&off.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
        }
        let cd = out.len() as u32;
        out.extend_from_slice(&central);
        out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(central.len() as u32).to_le_bytes());
        out.extend_from_slice(&cd.to_le_bytes());
        out.extend_from_slice(&[0; 2]);
        out
    }

    /// A zip of stored entries.
    pub fn store(files: &[(&str, &[u8])]) -> Vec<u8> {
        let v: Vec<(&str, &[u8], u32, u16)> = files.iter().map(|(n, d)| (*n, *d, d.len() as u32, 0)).collect();
        build(&v)
    }

    #[test]
    fn a_truncated_zip_is_an_error() {
        let z = store(&[("a.txt", b"hello")]);
        assert_eq!(Zip::open(&z).unwrap().read("a.txt").unwrap(), b"hello");
        for cut in [4, 20, z.len() - 10] {
            let r = Zip::open(&z[..cut]).and_then(|zip| zip.read("a.txt"));
            assert!(r.is_err(), "cut at {cut} read {r:?}");
        }
    }
}
