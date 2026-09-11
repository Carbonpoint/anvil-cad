//! Font catalogue for the Text feature.
//!
//! Two sources:
//! * Bundled DejaVu fonts, stored as `builtin:<name>`. They work on every
//!   machine, so documents that use them open the same everywhere.
//! * Fonts installed on the computer, found by scanning the usual font
//!   folders on Windows, macOS, and Linux. Stored as a file path.
//!
//! The scan runs once, the first time the list is asked for.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontEntry {
    /// Family name, for example "Arial".
    pub family: String,
    /// Style name, for example "Bold Italic".
    pub style: String,
    /// Value stored in the Text feature: `builtin:<name>` or a file path.
    pub source: String,
    pub builtin: bool,
}

impl FontEntry {
    pub fn label(&self) -> String {
        if self.style.is_empty()
            || self.style.eq_ignore_ascii_case("regular")
            || self.style.eq_ignore_ascii_case("book")
        {
            self.family.clone()
        } else {
            format!("{} {}", self.family, self.style)
        }
    }
}

/// Bundled fonts, heaviest first after the default. For 3D printing with a
/// 0.4 mm nozzle, strokes should be at least 0.8 mm wide, so small text
/// wants Archivo Black or a bold face.
pub const BUILTIN: &[(&str, &[u8])] = &[
    ("DejaVu Sans", include_bytes!("../assets/DejaVuSans.ttf")),
    ("Archivo Black", include_bytes!("../assets/ArchivoBlack-Regular.ttf")),
    ("Liberation Sans Bold", include_bytes!("../assets/LiberationSans-Bold.ttf")),
    ("DejaVu Sans Bold", include_bytes!("../assets/DejaVuSans-Bold.ttf")),
    ("DejaVu Serif", include_bytes!("../assets/DejaVuSerif.ttf")),
    ("DejaVu Serif Bold", include_bytes!("../assets/DejaVuSerif-Bold.ttf")),
    ("DejaVu Sans Mono", include_bytes!("../assets/DejaVuSansMono.ttf")),
];

/// Font bytes for a stored source value. Empty means DejaVu Sans.
pub fn load(source: &str) -> Result<std::borrow::Cow<'static, [u8]>, String> {
    let s = source.trim();
    if s.is_empty() {
        return Ok(std::borrow::Cow::Borrowed(BUILTIN[0].1));
    }
    if let Some(name) = s.strip_prefix("builtin:") {
        return BUILTIN
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, b)| std::borrow::Cow::Borrowed(*b))
            .ok_or_else(|| format!("no bundled font named {name}"));
    }
    std::fs::read(s).map(std::borrow::Cow::Owned).map_err(|e| format!("font file {s}: {e}"))
}

/// Human-readable name for a stored source value.
pub fn display_name(source: &str) -> String {
    let s = source.trim();
    if s.is_empty() {
        return "DejaVu Sans".into();
    }
    if let Some(name) = s.strip_prefix("builtin:") {
        return name.into();
    }
    catalogue()
        .iter()
        .find(|f| f.source == s)
        .map(|f| f.label())
        .unwrap_or_else(|| Path::new(s).file_stem().and_then(|x| x.to_str()).unwrap_or(s).to_string())
}

/// Every available font, bundled first, then installed fonts by name.
pub fn catalogue() -> &'static [FontEntry] {
    static CAT: OnceLock<Vec<FontEntry>> = OnceLock::new();
    CAT.get_or_init(|| {
        let mut out: Vec<FontEntry> = BUILTIN
            .iter()
            .map(|(n, _)| FontEntry {
                family: n.to_string(),
                style: String::new(),
                source: format!("builtin:{n}"),
                builtin: true,
            })
            .collect();
        let mut system = scan_system();
        system.sort_by_key(|f| f.label().to_lowercase());
        system.dedup_by(|a, b| a.label() == b.label());
        out.extend(system);
        out
    })
}

fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    if cfg!(windows) {
        if let Some(w) = env("WINDIR") {
            dirs.push(w.join("Fonts"));
        } else {
            dirs.push(PathBuf::from(r"C:\Windows\Fonts"));
        }
        if let Some(l) = env("LOCALAPPDATA") {
            dirs.push(l.join(r"Microsoft\Windows\Fonts"));
        }
    } else if cfg!(target_os = "macos") {
        dirs.push("/System/Library/Fonts".into());
        dirs.push("/Library/Fonts".into());
        if let Some(h) = env("HOME") {
            dirs.push(h.join("Library/Fonts"));
        }
    } else {
        dirs.push("/usr/share/fonts".into());
        dirs.push("/usr/local/share/fonts".into());
        if let Some(h) = env("HOME") {
            dirs.push(h.join(".local/share/fonts"));
            dirs.push(h.join(".fonts"));
        }
    }
    dirs
}

fn scan_system() -> Vec<FontEntry> {
    let mut out = Vec::new();
    let mut stack = font_dirs();
    let mut visited = 0usize;
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            if !matches!(ext.as_str(), "ttf" | "otf") {
                continue;
            }
            visited += 1;
            if visited > 5000 {
                return out;
            }
            if let Some(f) = describe(&path) {
                out.push(f);
            }
        }
    }
    out
}

/// Read family and style names from a font file.
fn describe(path: &Path) -> Option<FontEntry> {
    let data = std::fs::read(path).ok()?;
    let face = ttf_parser::Face::parse(&data, 0).ok()?;
    // Fonts without outlines for Latin letters are no use for engraving.
    face.glyph_index('A')?;
    let name = |ids: &[u16]| -> Option<String> {
        for id in ids {
            for n in face.names() {
                if n.name_id == *id && n.is_unicode() {
                    if let Some(s) = n.to_string() {
                        if !s.trim().is_empty() {
                            return Some(s);
                        }
                    }
                }
            }
        }
        None
    };
    // 16/17 are the typographic family and subfamily; 1/2 the legacy ones.
    let family = name(&[16, 1])?;
    let style = name(&[17, 2]).unwrap_or_default();
    Some(FontEntry { family, style, source: path.to_string_lossy().into_owned(), builtin: false })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_load_and_have_glyphs() {
        for (name, _) in BUILTIN {
            let bytes = load(&format!("builtin:{name}")).unwrap();
            let face = ttf_parser::Face::parse(&bytes, 0).unwrap();
            assert!(face.glyph_index('A').is_some(), "{name}");
        }
        assert_eq!(display_name(""), "DejaVu Sans");
        assert_eq!(display_name("builtin:DejaVu Serif"), "DejaVu Serif");
    }

    #[test]
    fn catalogue_starts_with_builtins() {
        let cat = catalogue();
        assert!(cat.len() >= BUILTIN.len());
        assert!(cat[0].builtin);
    }
}
