//! This fork's Chinese interface font: HarmonyOS Sans SC, read from this machine at runtime.
//!
//! The faces are loaded, never embedded or shipped (AGENTS.md §1.4): the TTFs stay in the folder
//! the owner picked. `PDFCRAFT_UI_FONT_DIR` points at that folder; without it the default below
//! is used. `PDFCRAFT_SYSTEM_FONTS=0` turns the faces off (published screenshots do).
//!
//! CUSTOM(kelongyan): see Rule/fork-customization.md.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use egui::{FontData, FontDefinitions, FontFamily};

const DIR_ENV: &str = "PDFCRAFT_UI_FONT_DIR";
/// Largest file read; a font path is untrusted input (same cap as the system Han fallback).
const MAX_BYTES: u64 = 96 << 20;
/// 欢 (U+6B22): a Simplified-only character the SC faces carry.
const PROBE: char = '\u{6B22}';

const REGULAR: &str = "HarmonyOSSansSC-Regular";
const MEDIUM: &str = "HarmonyOSSansSC-Medium";
const BOLD: &str = "HarmonyOSSansSC-Bold";

/// The three interface weights, read once per process.
struct Faces {
    regular: Arc<FontData>,
    medium: Option<Arc<FontData>>,
    bold: Option<Arc<FontData>>,
}

/// Insert the local faces into every family, after its Latin face and before the CJK fallbacks:
/// Latin text keeps Inter (or JetBrains Mono), Chinese text draws in HarmonyOS Sans, and
/// anything the SC faces lack falls through to the existing chain.
pub(crate) fn apply(fonts: &mut FontDefinitions) {
    let Some(faces) = faces() else { return };
    fonts.font_data.insert(REGULAR.to_owned(), faces.regular.clone());
    insert_after(fonts, FontFamily::Proportional, "Inter", REGULAR);
    insert_after(fonts, FontFamily::Monospace, "JetBrainsMono", REGULAR);
    let medium_name = match &faces.medium {
        Some(data) => {
            fonts.font_data.insert(MEDIUM.to_owned(), data.clone());
            MEDIUM
        }
        None => REGULAR,
    };
    insert_after(fonts, FontFamily::Name("medium".into()), "Inter-Medium", medium_name);
    let bold_name = match &faces.bold {
        Some(data) => {
            fonts.font_data.insert(BOLD.to_owned(), data.clone());
            BOLD
        }
        None => medium_name,
    };
    insert_after(fonts, FontFamily::Name("semibold".into()), "Inter-SemiBold", bold_name);
}

fn faces() -> Option<&'static Faces> {
    static CACHE: OnceLock<Option<Faces>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            let dir = font_dir()?;
            Some(Faces {
                regular: load(&dir.join("HarmonyOS_Sans_SC_Regular.ttf"))?,
                medium: load(&dir.join("HarmonyOS_Sans_SC_Medium.ttf")),
                bold: load(&dir.join("HarmonyOS_Sans_SC_Bold.ttf")),
            })
        })
        .as_ref()
}

fn insert_after(fonts: &mut FontDefinitions, family: FontFamily, anchor: &str, name: &str) {
    let Some(stack) = fonts.families.get_mut(&family) else { return };
    if stack.iter().any(|n| n == name) {
        return;
    }
    let at = stack.iter().position(|n| n == anchor).map_or(stack.len(), |i| i + 1);
    stack.insert(at, name.to_owned());
}

/// The folder holding the TTFs: `PDFCRAFT_UI_FONT_DIR`, else the owner's default location.
fn font_dir() -> Option<PathBuf> {
    if std::env::var_os("PDFCRAFT_SYSTEM_FONTS").is_some_and(|v| v == "0") {
        return None;
    }
    if let Some(dir) = std::env::var_os(DIR_ENV) {
        let dir = PathBuf::from(dir);
        return dir.is_dir().then_some(dir);
    }
    let home = std::env::var_os("USERPROFILE")?;
    let dir = Path::new(&home).join("Downloads").join("HarmonyOS Sans").join("HarmonyOS_Sans_SC");
    dir.is_dir().then_some(dir)
}

fn load(path: &Path) -> Option<Arc<FontData>> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let index = first_face_with(&bytes, PROBE)?;
    let mut data = FontData::from_owned(bytes);
    data.index = index;
    log::info!("interface font: {} (face {index})", path.display());
    Some(Arc::new(data))
}

/// The first face of the file that parses and maps `c` (egui parses fonts with the same skrifa).
fn first_face_with(bytes: &[u8], c: char) -> Option<u32> {
    use skrifa::MetadataProvider as _;
    (0..4).find(|&index| skrifa::FontRef::from_index(bytes, index).is_ok_and(|font| font.charmap().map(c).is_some()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_wrong_files_are_refused() {
        assert!(load(Path::new("definitely/not/here.ttf")).is_none());
        let inter = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/fonts/Inter-Regular.ttf"));
        assert!(load(inter).is_none(), "a Latin face has no Han glyphs");
        assert!(first_face_with(b"", PROBE).is_none());
        assert!(first_face_with(b"not a font at all", PROBE).is_none());
    }

    #[test]
    fn insertion_lands_after_the_latin_face() {
        let mut fonts = FontDefinitions::default();
        let stack = fonts.families.entry(FontFamily::Proportional).or_default();
        stack.extend(["Inter".to_owned(), "Jp".to_owned()]);
        insert_after(&mut fonts, FontFamily::Proportional, "Inter", REGULAR);
        insert_after(&mut fonts, FontFamily::Proportional, "Inter", REGULAR);
        let stack = &fonts.families[&FontFamily::Proportional];
        let at = stack.iter().position(|n| n == REGULAR).expect("inserted");
        assert_eq!(stack[at - 1], "Inter", "right after Inter");
        assert_eq!(stack.iter().filter(|n| *n == REGULAR).count(), 1, "inserted once");
        assert_eq!(stack[at + 1], "Jp", "before the CJK fallbacks");
    }
}
