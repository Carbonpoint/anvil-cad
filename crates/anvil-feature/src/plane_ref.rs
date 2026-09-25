//! A reference to a plane: a datum, a plane Feature, or a flat face of a
//! Body.
//!
//! A face is stored by its geometry, never by a `FaceId`, because a
//! `FaceId` is a slot key that does not survive a Regenerate. On each
//! Regenerate the face is found again:
//!
//! 1. A face that lies on the stored plane is used.
//! 2. Otherwise, if the Document remembers the face this reference matched
//!    last time (its place in the face list, its normal and its area), the
//!    face that fits that memory best is used. This is how a reference
//!    follows the top of an Extrude when the distance changes.
//! 3. Otherwise the stored plane is used, and the Feature gets a note.
//!
//! After a match the Document writes the new plane back into the
//! reference, so a saved file always holds the last known plane.

use crate::document::{Document, RegenContext, RegenError};
use anvil_kernel::{Solid, Surface};
use anvil_math::{DVec3, Plane};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub enum PlaneRef {
    /// Nothing chosen yet.
    #[default]
    None,
    /// A datum plane by name: "XY", "XZ" or "YZ".
    Datum(String),
    /// A Feature whose output carries a plane (a sketch, an offset plane,
    /// a midplane, and so on).
    Feature(usize),
    /// A planar face of a Body, found again by its geometry.
    Face {
        /// Feature that made the Body.
        feature: usize,
        /// Which Body of that Feature, by index.
        body: usize,
        /// The plane as measured when the user picked the face.
        plane: Plane,
    },
}

/// The saved shapes a `PlaneRef` can have. Documents written before
/// `PlaneRef` existed hold plain text: "XY", "XZ", "YZ" or a Feature number.
#[derive(Deserialize)]
#[serde(untagged)]
enum Repr {
    Text(String),
    Tagged(Tagged),
}

#[derive(Deserialize)]
enum Tagged {
    None,
    Datum(String),
    Feature(usize),
    Face { feature: usize, body: usize, plane: Plane },
}

impl<'de> Deserialize<'de> for PlaneRef {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match Repr::deserialize(d)? {
            Repr::Text(s) => PlaneRef::parse(&s),
            Repr::Tagged(Tagged::None) => PlaneRef::None,
            Repr::Tagged(Tagged::Datum(s)) => PlaneRef::Datum(s),
            Repr::Tagged(Tagged::Feature(i)) => PlaneRef::Feature(i),
            Repr::Tagged(Tagged::Face { feature, body, plane }) => PlaneRef::Face { feature, body, plane },
        })
    }
}

impl From<&str> for PlaneRef {
    fn from(s: &str) -> Self {
        PlaneRef::parse(s)
    }
}

impl PlaneRef {
    /// Read the old text form: a datum name or a Feature number. Anything
    /// else is `None`.
    pub fn parse(s: &str) -> PlaneRef {
        let t = s.trim();
        match t.to_ascii_uppercase().as_str() {
            "XY" | "XZ" | "YZ" => PlaneRef::Datum(t.to_ascii_uppercase()),
            _ => t.parse::<usize>().map(PlaneRef::Feature).unwrap_or(PlaneRef::None),
        }
    }

    /// Combine the old saved pair: a choice that was "Feature" and the
    /// Feature number stored beside it.
    pub fn from_saved(r: PlaneRef, feature: Option<usize>) -> PlaneRef {
        match (r, feature) {
            (PlaneRef::None, Some(i)) => PlaneRef::Feature(i),
            (r, _) => r,
        }
    }

    /// Short text for the Properties panel, for example "XY",
    /// "3: Extrude" or "Face of 3: Extrude".
    pub fn label(&self, doc: &Document) -> String {
        let feature_name = |i: usize| match doc.features.get(i) {
            Some(n) => format!("{i}: {}", n.feature.name()),
            None => format!("{i}: (missing)"),
        };
        match self {
            PlaneRef::None => "(none)".into(),
            PlaneRef::Datum(s) => s.clone(),
            PlaneRef::Feature(i) => feature_name(*i),
            PlaneRef::Face { feature, body: 0, .. } => format!("Face of {}", feature_name(*feature)),
            PlaneRef::Face { feature, body, .. } => format!("Face of {}, body {}", feature_name(*feature), body + 1),
        }
    }

    /// Short text without Feature names, for a Feature's own name.
    pub fn short(&self) -> String {
        match self {
            PlaneRef::None => "?".into(),
            PlaneRef::Datum(s) => s.clone(),
            PlaneRef::Feature(i) => i.to_string(),
            PlaneRef::Face { feature, .. } => format!("face of {feature}"),
        }
    }

    /// True when this reference needs `feature` to exist.
    pub fn depends_on(&self) -> Option<usize> {
        match self {
            PlaneRef::Feature(i) | PlaneRef::Face { feature: i, .. } => Some(*i),
            _ => None,
        }
    }

    /// The same reference after the history changed. `None` when the
    /// Feature it names was deleted.
    pub fn remapped(&self, map: &dyn Fn(usize) -> Option<usize>) -> Option<PlaneRef> {
        Some(match self {
            PlaneRef::Feature(i) => PlaneRef::Feature(map(*i)?),
            PlaneRef::Face { feature, body, plane } => {
                PlaneRef::Face { feature: map(*feature)?, body: *body, plane: *plane }
            }
            other => other.clone(),
        })
    }
}

/// What the Document remembers about the face a reference matched on the
/// last Regenerate. Not saved; a loaded file matches by its stored plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceMemory {
    /// Place of the face in the Body's face list.
    pub ordinal: usize,
    pub normal: DVec3,
    pub area: f64,
}

/// Key of a `FaceMemory`: the Feature that holds the reference, and the
/// reference itself as it was stored.
pub type FaceKey = (usize, usize, usize, [u64; 9]);

pub(crate) fn face_key(holder: usize, feature: usize, body: usize, plane: &Plane) -> FaceKey {
    let v = [plane.origin, plane.x_axis, plane.y_axis];
    let mut bits = [0u64; 9];
    for (i, p) in v.iter().enumerate() {
        bits[i * 3] = p.x.to_bits();
        bits[i * 3 + 1] = p.y.to_bits();
        bits[i * 3 + 2] = p.z.to_bits();
    }
    (holder, feature, body, bits)
}

/// One planar face of a Body, measured.
struct FlatFace {
    ordinal: usize,
    normal: DVec3,
    area: f64,
    centre: DVec3,
}

fn flat_faces(solid: &Solid) -> Vec<FlatFace> {
    solid
        .faces
        .values()
        .enumerate()
        .filter(|(_, f)| f.surface == Surface::Plane)
        .filter_map(|(ordinal, f)| {
            let (normal, area) = solid.face_normal_area(f);
            if area <= 1e-12 || f.outer.is_empty() {
                return None;
            }
            let centre = f.outer.iter().map(|&v| solid.pos(v)).sum::<DVec3>() / f.outer.len() as f64;
            Some(FlatFace { ordinal, normal, area, centre })
        })
        .collect()
}

/// The plane of `face`, oriented like `stored`: the stored normal's side,
/// the stored x axis projected onto the face, the stored origin moved onto
/// the face along the normal.
fn plane_on_face(face: &FlatFace, stored: &Plane) -> Plane {
    let n = if face.normal.dot(stored.normal()) < 0.0 { -face.normal } else { face.normal };
    let mut x = (stored.x_axis - n * stored.x_axis.dot(n)).normalize_or_zero();
    if x.length_squared() < 1e-12 {
        x = n.any_orthonormal_vector();
    }
    let origin = stored.origin - n * (stored.origin - face.centre).dot(n);
    Plane { origin, x_axis: x, y_axis: n.cross(x) }
}

/// Result of finding a face again.
pub(crate) struct FaceMatch {
    pub plane: Plane,
    /// The face that was used, if one was found.
    pub memory: Option<FaceMemory>,
}

/// Find the stored face in `solid`. See the module notes for the order.
pub(crate) fn find_face(solid: &Solid, stored: &Plane, memory: Option<&FaceMemory>) -> FaceMatch {
    let faces = flat_faces(solid);
    let n0 = stored.normal();
    let b = solid.bounds();
    let diag = (b.max - b.min).length().max(1e-9);
    let remember = |f: &FlatFace| FaceMemory { ordinal: f.ordinal, normal: f.normal, area: f.area };

    // 1. A face on the stored plane, either side.
    let on_plane = faces
        .iter()
        .filter(|f| f.normal.cross(n0).length() < 1e-6 && (f.centre - stored.origin).dot(n0).abs() < 1e-6 * diag)
        .min_by(|a, b| (a.centre - stored.origin).length().total_cmp(&(b.centre - stored.origin).length()));
    if let Some(f) = on_plane {
        return FaceMatch { plane: plane_on_face(f, stored), memory: Some(remember(f)) };
    }

    // 2. The face that fits the memory: same normal, then the same place
    //    in the face list, then the nearest area.
    if let Some(m) = memory {
        let same_normal: Vec<&FlatFace> = faces.iter().filter(|f| f.normal.dot(m.normal) > 1.0 - 1e-6).collect();
        let best = same_normal.iter().find(|f| f.ordinal == m.ordinal).copied().or_else(|| {
            same_normal
                .iter()
                .filter(|f| f.area > 0.5 * m.area && f.area < 2.0 * m.area)
                .min_by(|a, b| (a.area - m.area).abs().total_cmp(&(b.area - m.area).abs()))
                .copied()
        });
        if let Some(f) = best {
            return FaceMatch { plane: plane_on_face(f, stored), memory: Some(remember(f)) };
        }
    }

    // 3. Nothing fits.
    FaceMatch { plane: *stored, memory: None }
}

/// Note given to a Feature whose picked face was not found.
pub const MOVED_NOTE: &str = "the picked face moved, the last known plane is used";

impl RegenContext<'_> {
    /// The plane a reference names, for the Feature being regenerated.
    pub fn plane_of_ref(&self, r: &PlaneRef) -> Result<Plane, RegenError> {
        match r {
            PlaneRef::None => Err(RegenError::Other("no plane chosen".into())),
            PlaneRef::Datum(name) => match name.trim().to_ascii_uppercase().as_str() {
                n @ ("XY" | "XZ" | "YZ") => Ok(crate::features::datum_plane(n)),
                _ => Err(RegenError::Other(format!("'{name}' is not a datum plane; use XY, XZ or YZ"))),
            },
            PlaneRef::Feature(i) => self.plane_of(*i),
            PlaneRef::Face { feature, body, plane } => {
                let bodies = self.bodies_of(*feature)?;
                let solid = bodies.get(*body).ok_or(RegenError::BadReference(self.index, *feature, "body"))?;
                let key = face_key(self.index, *feature, *body, plane);
                let found = find_face(solid, plane, self.face_memory.get(&key));
                match found.memory {
                    Some(m) => self.face_hits.borrow_mut().push((r.clone(), found.plane, m)),
                    None => self.notes.borrow_mut().push(MOVED_NOTE.into()),
                }
                Ok(found.plane)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_text_reads_as_a_datum_or_a_feature() {
        let a: PlaneRef = serde_json::from_str("\"XY\"").unwrap();
        let b: PlaneRef = serde_json::from_str("\"3\"").unwrap();
        let c: PlaneRef = serde_json::from_str("\"None\"").unwrap();
        assert_eq!(a, PlaneRef::Datum("XY".into()));
        assert_eq!(b, PlaneRef::Feature(3));
        assert_eq!(c, PlaneRef::None);
    }

    #[test]
    fn every_shape_round_trips() {
        for r in [
            PlaneRef::None,
            PlaneRef::Datum("YZ".into()),
            PlaneRef::Feature(2),
            PlaneRef::Face { feature: 1, body: 0, plane: Plane::XZ },
        ] {
            let s = serde_json::to_string(&r).unwrap();
            let back: PlaneRef = serde_json::from_str(&s).unwrap();
            assert_eq!(back, r, "{s}");
        }
    }
}
