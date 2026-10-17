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

/// An `n`-page PDF whose pages say "Page i+1", so thumbnails show real content.
fn fixture_pdf(n: usize) -> Vec<u8> {
    let mut objs: Vec<String> = vec!["<< /Type /Catalog /Pages 2 0 R >>".into()];
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 4 + 2 * i)).collect();
    objs.push(format!("<< /Type /Pages /Kids [{}] /Count {n} /MediaBox [0 0 200 300] >>", kids.join(" ")));
    objs.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into());
    for i in 0..n {
        objs.push(format!("<< /Type /Page /Parent 2 0 R /Contents {} 0 R /Resources << /Font << /F1 3 0 R >> >> >>", 5 + 2 * i));
        let body = format!("BT /F1 24 Tf 20 150 Td (Page {}) Tj ET", i + 1);
        objs.push(format!("<< /Length {} >>\nstream\n{body}\nendstream", body.len()));
    }
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for o in offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes());
    out
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

#[test]
fn home_cards_get_first_page_thumbnails() {
    let file = std::env::temp_dir().join(format!("pdfcraft-thumb-{}.pdf", std::process::id()));
    std::fs::write(&file, fixture_pdf(1)).unwrap();
    let path = file.to_string_lossy().into_owned();
    let shown = path.clone();
    let mut h = harness(move |app| {
        app.library.remember(&shown, "one.pdf", 1, 100, 1);
    });
    let mut ready = false;
    for _ in 0..600 {
        h.run_steps(2);
        if h.state().thumbnails.is_ready(&path) {
            ready = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(ready, "the first page renders into a texture");
    let _ = std::fs::remove_file(&file);
}

/// Saves review shots when `PDFCRAFT_LIBRARY_SHOTS` points at a directory.
#[test]
fn library_shots_for_review() {
    let Ok(dir) = std::env::var("PDFCRAFT_LIBRARY_SHOTS") else { return };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let day = 86_400;
    // Real files on disk, so the cards render their first pages.
    let files = std::env::temp_dir().join(format!("pdfcraft-library-shots-{}", std::process::id()));
    std::fs::create_dir_all(&files).unwrap();
    let (report, contract, manual) = (files.join("report.pdf"), files.join("contract.pdf"), files.join("manual.pdf"));
    std::fs::write(&report, fixture_pdf(12)).unwrap();
    std::fs::write(&contract, fixture_pdf(3)).unwrap();
    std::fs::write(&manual, fixture_pdf(48)).unwrap();
    let (r, c, m) = (report.to_string_lossy().into_owned(), contract.to_string_lossy().into_owned(), manual.to_string_lossy().into_owned());
    let zh = |h: &mut Harness<'static, PdfCraftApp>| {
        h.state_mut().set_option("language", "zh-hans").unwrap();
        h.run_steps(3);
    };
    let (r_in, c_in, m_in) = (r.clone(), c.clone(), m.clone());
    let mut h = harness(move |app| {
        let bills = app.library.create_folder("申论").unwrap();
        app.library.create_folder("行测");
        let tax = app.library.create_folder("历年真题").unwrap();
        app.library.set_folder_starred(tax, true);
        app.library.remember(&r_in, "报告-2026-Q3.pdf", 12, 2_400_000, now.saturating_sub(day * 2));
        app.library.remember(&c_in, "合同扫描件.pdf", 3, 2_400_000, now.saturating_sub(day / 2));
        app.library.remember(&m_in, "论文终稿 v7.pdf", 48, 2_400_000, now.saturating_sub(day * 9));
        app.library.move_doc(&r_in, Some(bills));
    });
    zh(&mut h);
    // Wait for the first-page thumbnails before the shot.
    for _ in 0..600 {
        h.run_steps(2);
        if [&r, &c, &m].iter().all(|path| h.state().thumbnails.is_ready(path)) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    h.render().unwrap().save(format!("{dir}/library-full.png")).unwrap();
    let mut empty = harness(|_| {});
    zh(&mut empty);
    empty.render().unwrap().save(format!("{dir}/library-empty.png")).unwrap();
    let _ = std::fs::remove_dir_all(&files);
}
