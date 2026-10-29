//! The document library behind this fork's Home view: user-created folders over the PDFs the app
//! has opened. A reference list only — nothing here copies, moves or deletes a file on disk.
//!
//! CUSTOM(kelongyan): part of the fork's Home (custom/home.rs); see Rule/fork-customization.md.

use serde::Serialize;

/// Safety caps: a hostile or corrupt settings file cannot grow the library without bound.
const MAX_FOLDERS: usize = 200;
const MAX_DOCS: usize = 1000;
/// Longest kept folder name, in characters.
const MAX_NAME_CHARS: usize = 60;

/// A colour a folder can wear on Home. `base` is the hue its light and dark tints are derived
/// from; `name` is the tooltip's translation key. Index 0 is the accent default, so a folder made
/// before colours existed keeps the tint it always had.
pub struct FolderColor {
    pub name: &'static str,
    pub base: [u8; 3],
}

/// Home's folder palette: three calm rows of eight. Indices 0-7 are the original colours, so a
/// folder made before the wider palette keeps the tint it always had; 8-15 are deeper, 16-23 pastel.
pub const FOLDER_PALETTE: [FolderColor; 24] = [
    FolderColor { name: "Default", base: [0x1B, 0x63, 0xE0] },
    FolderColor { name: "Slate", base: [0x6B, 0x72, 0x80] },
    FolderColor { name: "Teal", base: [0x0F, 0x8A, 0x8A] },
    FolderColor { name: "Green", base: [0x2F, 0x8A, 0x4B] },
    FolderColor { name: "Amber", base: [0xB0, 0x7C, 0x14] },
    FolderColor { name: "Orange", base: [0xC2, 0x54, 0x2A] },
    FolderColor { name: "Rose", base: [0xC2, 0x43, 0x6B] },
    FolderColor { name: "Violet", base: [0x7A, 0x5B, 0xD1] },
    FolderColor { name: "Wine", base: [0x7E, 0x2F, 0x3A] },
    FolderColor { name: "Rust", base: [0xA3, 0x4E, 0x2A] },
    FolderColor { name: "Olive", base: [0x7A, 0x7A, 0x22] },
    FolderColor { name: "Forest", base: [0x1F, 0x6B, 0x3A] },
    FolderColor { name: "Navy", base: [0x1F, 0x4E, 0x8C] },
    FolderColor { name: "Plum", base: [0x5B, 0x3A, 0x82] },
    FolderColor { name: "Berry", base: [0x8A, 0x2B, 0x5E] },
    FolderColor { name: "Graphite", base: [0x4B, 0x4F, 0x57] },
    FolderColor { name: "Pastel Rose", base: [0xE4, 0x8A, 0x9A] },
    FolderColor { name: "Peach", base: [0xE8, 0xA9, 0x7E] },
    FolderColor { name: "Butter", base: [0xE6, 0xC8, 0x6E] },
    FolderColor { name: "Mint", base: [0x8F, 0xCF, 0xA6] },
    FolderColor { name: "Sky", base: [0x8F, 0xBD, 0xE4] },
    FolderColor { name: "Lilac", base: [0xB8, 0xA6, 0xE0] },
    FolderColor { name: "Blush", base: [0xE4, 0xA6, 0xC4] },
    FolderColor { name: "Cloud", base: [0xC9, 0xC6, 0xBE] },
];

#[derive(Clone, Debug, PartialEq, serde::Deserialize, Serialize)]
pub struct LibraryFolder {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub starred: bool,
    /// Index into [`FOLDER_PALETTE`]; 0 is the accent default.
    #[serde(default)]
    pub color: u8,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, Serialize)]
pub struct LibraryDoc {
    pub path: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub folder: Option<u64>,
    /// Unix seconds when the app first remembered the document.
    #[serde(default)]
    pub added: u64,
    #[serde(default)]
    pub pages: usize,
    #[serde(default)]
    pub size: usize,
    /// The file was missing the last time it was checked.
    #[serde(default)]
    pub missing: bool,
}

/// Folders and remembered documents. Documents are kept newest first.
#[derive(Debug, Default, Serialize)]
pub struct Library {
    folders: Vec<LibraryFolder>,
    docs: Vec<LibraryDoc>,
    #[serde(skip)]
    next_id: u64,
}

impl Library {
    pub fn folders(&self) -> &[LibraryFolder] {
        &self.folders
    }

    pub fn folder(&self, id: u64) -> Option<&LibraryFolder> {
        self.folders.iter().find(|f| f.id == id)
    }

    pub fn docs(&self) -> &[LibraryDoc] {
        &self.docs
    }

    /// Documents in a folder; `None` is the "All" top level, which shows only unfiled documents.
    pub fn docs_in(&self, folder: Option<u64>) -> impl Iterator<Item = &LibraryDoc> {
        self.docs.iter().filter(move |d| d.folder == folder)
    }

    pub fn folder_doc_count(&self, id: u64) -> usize {
        self.docs.iter().filter(|d| d.folder == Some(id)).count()
    }

    /// The newest `added` stamp among a folder's documents.
    pub fn folder_newest_added(&self, id: u64) -> Option<u64> {
        self.docs.iter().filter(|d| d.folder == Some(id)).map(|d| d.added).max()
    }

    pub fn is_empty(&self) -> bool {
        self.folders.is_empty() && self.docs.is_empty()
    }

    /// Remember a document the user imported. An existing entry keeps its folder and only
    /// refreshes the file facts. Only an import calls this: a PDF that was merely opened stays out
    /// of Home (see [`Self::refresh`]).
    pub fn remember(&mut self, path: &str, name: &str, pages: usize, size: usize, now: u64) {
        if let Some(d) = self.docs.iter_mut().find(|d| d.path == path) {
            d.name = name.to_string();
            d.pages = pages;
            d.size = size;
            d.missing = false;
            return;
        }
        if self.docs.len() >= MAX_DOCS {
            return;
        }
        self.docs.insert(0, LibraryDoc { path: path.to_string(), name: name.to_string(), folder: None, added: now, pages, size, missing: false });
    }

    /// Bring a document the library already holds up to date after it was opened again. A file the
    /// library does not hold is left out: Home shows what was imported, not everything opened.
    pub fn refresh(&mut self, path: &str, name: &str, pages: usize, size: usize) {
        if let Some(d) = self.docs.iter_mut().find(|d| d.path == path) {
            d.name = name.to_string();
            d.pages = pages;
            d.size = size;
            d.missing = false;
        }
    }

    /// Create a folder with the colour it will keep: a folder's colour is chosen here and cannot
    /// be changed afterwards.
    pub fn create_folder(&mut self, name: &str, color: u8) -> Option<u64> {
        let name = clean_name(name)?;
        if self.folders.len() >= MAX_FOLDERS {
            return None;
        }
        let color = if usize::from(color) < FOLDER_PALETTE.len() { color } else { 0 };
        self.next_id = self.next_id.max(1);
        let id = self.next_id;
        self.next_id += 1;
        self.folders.push(LibraryFolder { id, name, starred: false, color });
        Some(id)
    }

    pub fn rename_folder(&mut self, id: u64, name: &str) {
        let Some(name) = clean_name(name) else { return };
        if let Some(f) = self.folders.iter_mut().find(|f| f.id == id) {
            f.name = name;
        }
    }

    pub fn set_folder_starred(&mut self, id: u64, starred: bool) {
        if let Some(f) = self.folders.iter_mut().find(|f| f.id == id) {
            f.starred = starred;
        }
    }

    /// Remove a folder. Its documents are not deleted: they move back to "All".
    pub fn delete_folder(&mut self, id: u64) {
        for d in &mut self.docs {
            if d.folder == Some(id) {
                d.folder = None;
            }
        }
        self.folders.retain(|f| f.id != id);
    }

    /// File a document into a folder; `None` moves it back to "All".
    pub fn move_doc(&mut self, path: &str, folder: Option<u64>) {
        let folder = folder.filter(|id| self.folders.iter().any(|f| f.id == *id));
        if let Some(d) = self.docs.iter_mut().find(|d| d.path == path) {
            d.folder = folder;
        }
    }

    pub fn remove_doc(&mut self, path: &str) {
        self.docs.retain(|d| d.path != path);
    }

    /// Point a remembered document at its new location, after the user located it again.
    pub fn relocate(&mut self, old_path: &str, new_path: &str, new_name: &str) {
        if let Some(d) = self.docs.iter_mut().find(|d| d.path == old_path) {
            d.path = new_path.to_string();
            d.name = new_name.to_string();
            d.missing = false;
        }
    }

    /// Record the result of an existence check the caller made.
    pub fn set_missing(&mut self, path: &str, missing: bool) {
        if let Some(d) = self.docs.iter_mut().find(|d| d.path == path) {
            d.missing = missing;
        }
    }

    /// Restore from settings. Tolerant by design: a damaged or hostile value yields whatever
    /// still parses, and never an error the caller must handle.
    pub fn restore(&mut self, value: &serde_json::Value) {
        self.folders.clear();
        self.docs.clear();
        self.next_id = 0;
        if let Ok(folders) = serde_json::from_value::<Vec<LibraryFolder>>(value["folders"].clone()) {
            for f in folders.into_iter().take(MAX_FOLDERS) {
                let Some(name) = clean_name(&f.name) else { continue };
                if f.id == 0 || self.folders.iter().any(|g| g.id == f.id) {
                    continue;
                }
                self.next_id = self.next_id.max(f.id + 1);
                // Settings are untrusted: a colour the palette no longer has falls back to the
                // accent default rather than inventing a swatch.
                let color = if usize::from(f.color) < FOLDER_PALETTE.len() { f.color } else { 0 };
                self.folders.push(LibraryFolder { id: f.id, name, starred: f.starred, color });
            }
        }
        if let Ok(docs) = serde_json::from_value::<Vec<LibraryDoc>>(value["docs"].clone()) {
            for mut d in docs.into_iter().take(MAX_DOCS) {
                if d.path.is_empty() || self.docs.iter().any(|g| g.path == d.path) {
                    continue;
                }
                if d.name.trim().is_empty() {
                    d.name = std::path::Path::new(&d.path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| d.path.clone());
                }
                d.folder = d.folder.filter(|id| self.folders.iter().any(|f| f.id == *id));
                #[cfg(not(target_arch = "wasm32"))]
                {
                    d.missing = !std::path::Path::new(&d.path).exists();
                }
                self.docs.push(d);
            }
        }
    }
}

/// A trimmed, non-empty, length-capped folder name.
fn clean_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(MAX_NAME_CHARS).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_and_documents_round_trip_through_settings() {
        let mut lib = Library::default();
        let id = lib.create_folder("  Bills  ", 0).unwrap();
        lib.remember("/nowhere/a.pdf", "a.pdf", 3, 100, 7);
        lib.remember("/nowhere/b.pdf", "b.pdf", 1, 10, 9);
        lib.move_doc("/nowhere/a.pdf", Some(id));
        lib.set_folder_starred(id, true);
        assert_eq!(lib.folder_doc_count(id), 1);
        assert_eq!(lib.docs_in(None).count(), 1);
        let saved = serde_json::to_value(&lib).unwrap();
        let mut back = Library::default();
        back.restore(&saved);
        assert_eq!(back.folders().len(), 1);
        assert!(back.folders()[0].starred);
        assert_eq!(back.docs_in(Some(id)).count(), 1);
        // The next folder id continues past the restored ones.
        assert_eq!(back.create_folder("More", 0), Some(id + 1));
    }

    #[test]
    fn a_folders_colour_survives_settings_and_an_unknown_one_returns_to_the_default() {
        let mut lib = Library::default();
        let id = lib.create_folder("Work", 3).unwrap();
        assert_eq!(lib.folder(id).unwrap().color, 3, "a new folder keeps the colour chosen at creation");
        let saved = serde_json::to_value(&lib).unwrap();
        let mut back = Library::default();
        back.restore(&saved);
        assert_eq!(back.folder(id).unwrap().color, 3);
        // Settings a newer build wrote: an index the palette does not have is the default, and a
        // folder saved before colours existed carries none.
        back.restore(&serde_json::json!({"folders": [{"id": 7, "name": "Old", "color": 200}]}));
        assert_eq!(back.folder(7).unwrap().color, 0);
        back.restore(&serde_json::json!({"folders": [{"id": 8, "name": "Older"}]}));
        assert_eq!(back.folder(8).unwrap().color, 0);
    }

    #[test]
    fn damaged_or_hostile_settings_degrade_to_what_parses() {
        let mut lib = Library::default();
        lib.restore(&serde_json::json!({
            "folders": [{"id": 0, "name": "  "}, {"id": 4, "name": "Kept"}, {"id": 4, "name": "Dup"}],
            "docs": [{"path": ""}, {"path": "/nowhere/a.pdf", "folder": 4}, {"path": "/nowhere/a.pdf"}],
        }));
        assert_eq!(lib.folders().len(), 1);
        assert_eq!(lib.folders()[0].name, "Kept");
        assert_eq!(lib.docs().len(), 1);
        assert_eq!(lib.docs()[0].folder, Some(4));
        // A missing folder is dropped from a document, and a stale id in settings too.
        let mut lib = Library::default();
        lib.restore(&serde_json::json!({"docs": [{"path": "/nowhere/a.pdf", "folder": 9}]}));
        assert_eq!(lib.docs()[0].folder, None);
    }

    #[test]
    fn deleting_a_folder_returns_its_documents_to_all() {
        let mut lib = Library::default();
        let id = lib.create_folder("Scans", 0).unwrap();
        lib.remember("/nowhere/a.pdf", "a.pdf", 1, 1, 1);
        lib.move_doc("/nowhere/a.pdf", Some(id));
        lib.delete_folder(id);
        assert!(lib.folders().is_empty());
        assert_eq!(lib.docs_in(None).count(), 1, "the document is not deleted");
        // Moving to a folder that does not exist falls back to All.
        lib.move_doc("/nowhere/a.pdf", Some(42));
        assert_eq!(lib.docs()[0].folder, None);
    }
}
