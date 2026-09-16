use std::path::{Path, PathBuf};

use hive_protocol::{DirListing, FsEntry};

pub fn list_dir(path: &str) -> Result<DirListing, String> {
    let p = if path.is_empty() {
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into()))
    } else {
        PathBuf::from(path)
    };
    let p = if p.is_absolute() {
        p
    } else {
        Path::new(&std::env::var("HOME").unwrap_or_default()).join(p)
    };
    if !p.is_dir() {
        return Err(format!("not a directory: {}", p.display()));
    }
    let mut entries = Vec::new();
    for ent in std::fs::read_dir(&p).map_err(|e| e.to_string())? {
        let ent = ent.map_err(|e| e.to_string())?;
        let name = ent.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        entries.push(FsEntry {
            is_dir: ent.path().is_dir(),
            name,
        });
    }
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });
    Ok(DirListing {
        path: p.to_string_lossy().into_owned(),
        entries,
    })
}
