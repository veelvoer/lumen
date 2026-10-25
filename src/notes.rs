//! Patch notes that ship inside the app.

include!(concat!(env!("OUT_DIR"), "/notes_gen.rs"));

fn parts(v: &str) -> Vec<u64> {
    v.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

/// Notes of every version after `seen` up to and including `now`, newest first, as plain lines.
pub fn since(seen: &str, now: &str) -> Vec<(String, Vec<String>)> {
    let mut v: Vec<(String, Vec<String>)> = NOTES
        .iter()
        .filter(|(ver, _)| parts(ver) <= parts(now) && (seen.is_empty() || parts(ver) > parts(seen)))
        .map(|(ver, text)| (ver.to_string(), crate::update::plain(text).into_iter().skip(1).collect()))
        .filter(|(_, lines): &(String, Vec<String>)| !lines.is_empty())
        .collect();
    v.sort_by(|a, b| parts(&b.0).cmp(&parts(&a.0)));
    v
}
