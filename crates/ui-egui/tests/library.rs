//! CUSTOM(kelongyan): the fork's document library on Home (custom/home.rs, custom/library.rs):
//! creating folders, remembering opened PDFs, and the library surviving a restart.

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use pdfcraft_ui_egui::PdfCraftApp;

/// A one-page PDF.
const FIXTURE: &[u8] = b"%PDF-1.7
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] >> endobj
trailer << /Root 1 0 R >>
%%EOF";

fn harness(setup: impl FnOnce(&mut PdfCraftApp) + 'static) -> Harness<'static, PdfCraftApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(move |_cc| {
        let mut app = PdfCraftApp::new();
        app.set_option("language", "en").unwrap();
        // Open and settings work on the frame that asks, not on a worker thread.
        app.run_inline = true;
        setup(&mut app);
        app
    });
    // Fonts install on frame 1 and apply on frame 2.
    h.run_steps(4);
    h
}

#[test]
fn the_empty_library_invites_a_drop_or_a_new_folder() {
    let h = harness(|_| {});
    h.get_by_label("Documents");
    h.get_by_label("Open file");
    h.get_by_label("New folder");
    h.get_by_label_contains("Drop a PDF anywhere to open it");
}

#[test]
fn creating_a_folder_shows_it_on_home_and_it_survives_a_restart() {
    let mut h = harness(|_| {});
    h.get_by_label("New folder").click();
    h.run_steps(2);
    h.get_by_role_and_label(Role::TextInput, "New folder").type_text("Bills");
    h.run_steps(2);
    h.get_by_label("Create").click();
    h.run_steps(2);
    h.get_by_label("Bills");
    assert_eq!(h.state().library.folders().len(), 1);
    // The library rides in the saved settings.
    let mut restarted = PdfCraftApp::new();
    restarted.set_option("language", "en").unwrap();
    restarted.restore(&h.state().persist());
    assert_eq!(restarted.library.folders().len(), 1);
    assert_eq!(restarted.library.folders()[0].name, "Bills");
}

#[test]
fn an_opened_pdf_is_remembered_on_home() {
    let mut h = harness(|app| {
        app.open_bytes("report.pdf", Some("Z:/nowhere/report.pdf".into()), FIXTURE.to_vec()).unwrap();
    });
    assert_eq!(h.state().library.docs().len(), 1);
    assert_eq!(h.state().library.docs()[0].name, "report.pdf");
    // Closing the document returns to Home, where the card shows.
    h.state_mut().execute("file.close_all");
    h.run_steps(2);
    h.get_by_label("Documents");
    h.get_by_label("report.pdf");
}

#[test]
fn renaming_a_folder_updates_its_card() {
    let mut h = harness(|app| {
        app.library.create_folder("Old name");
    });
    h.get_by_label("Old name");
    h.state_mut().library.rename_folder(1, "New name");
    h.run_steps(2);
    h.get_by_label("New name");
    assert!(h.query_by_label("Old name").is_none());
}

/// Saves review shots when `PDFCRAFT_LIBRARY_SHOTS` points at a directory.
#[test]
fn library_shots_for_review() {
    let Ok(dir) = std::env::var("PDFCRAFT_LIBRARY_SHOTS") else { return };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let day = 86_400;
    let zh = |h: &mut Harness<'static, PdfCraftApp>| {
        h.state_mut().set_option("language", "zh-hans").unwrap();
        h.run_steps(3);
    };
    let mut h = harness(move |app| {
        let bills = app.library.create_folder("申论").unwrap();
        app.library.create_folder("行测");
        let tax = app.library.create_folder("历年真题").unwrap();
        app.library.set_folder_starred(tax, true);
        for (path, name, pages, age) in [
            ("Z:/nowhere/report.pdf", "报告-2026-Q3.pdf", 12usize, day * 2),
            ("Z:/nowhere/contract.pdf", "合同扫描件.pdf", 3, day / 2),
            ("Z:/nowhere/manual.pdf", "论文终稿 v7.pdf", 48, day * 9),
        ] {
            app.library.remember(path, name, pages, 2_400_000, now.saturating_sub(age));
        }
        app.library.move_doc("Z:/nowhere/report.pdf", Some(bills));
    });
    zh(&mut h);
    h.render().unwrap().save(format!("{dir}/library-full.png")).unwrap();
    let mut empty = harness(|_| {});
    zh(&mut empty);
    empty.render().unwrap().save(format!("{dir}/library-empty.png")).unwrap();
}
