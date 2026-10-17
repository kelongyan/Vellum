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

#[derive(Clone, Debug, PartialEq, serde::Deserialize, Serialize)]
pub struct LibraryFolder {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub starred: bool,
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

    /// Remember a document that was just opened. An existing entry keeps its folder and only
    /// refreshes the file facts.
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

    pub fn create_folder(&mut self, name: &str) -> Option<u64> {
        let name = clean_name(name)?;
        if self.folders.len() >= MAX_FOLDERS {
            return None;
        }
        self.next_id = self.next_id.max(1);
        let id = self.next_id;
        self.next_id += 1;
        self.folders.push(LibraryFolder { id, name, starred: false });
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
                self.folders.push(LibraryFolder { id: f.id, name, starred: f.starred });
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
        let id = lib.create_folder("  Bills  ").unwrap();
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
        assert_eq!(back.create_folder("More"), Some(id + 1));
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
        let id = lib.create_folder("Scans").unwrap();
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
