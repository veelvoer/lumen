// Packs the release notes into the app, so they can be shown after an update without a network.
use std::fmt::Write;

fn main() {
    let dir = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("release-notes");
    println!("cargo:rerun-if-changed=release-notes");
    let mut files: Vec<_> = std::fs::read_dir(&dir).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect()).unwrap_or_default();
    files.retain(|p| p.extension().is_some_and(|e| e == "md"));
    files.sort();
    let mut out = String::from("pub const NOTES: &[(&str, &str)] = &[\n");
    for f in files {
        let v = f.file_stem().unwrap().to_string_lossy().to_string();
        let _ = writeln!(out, "    ({v:?}, include_str!({:?})),", f.to_string_lossy());
    }
    out.push_str("];\n");
    std::fs::write(std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("notes_gen.rs"), out).unwrap();
}
