use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Labels {
    #[serde(default)]
    pub starred: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl Default for Labels {
    fn default() -> Self {
        Self {
            starred: false,
            tags: Vec::new(),
        }
    }
}

pub fn load(path: &Path) -> Labels {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Labels::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn normalize_tag(raw: &str) -> Result<String, String> {
    let t = raw.trim().to_ascii_lowercase();
    if t.is_empty() {
        return Err("tag must not be empty".into());
    }
    if t.len() > 32 {
        return Err("tag too long (max 32)".into());
    }
    if !t
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("tag may only contain letters, digits, -, _".into());
    }
    Ok(t)
}

/// Sort, dedupe, and apply cross-tag rules (evaluation implies room).
pub fn normalize_tags(mut tags: Vec<String>) -> Vec<String> {
    tags.sort();
    tags.dedup();
    if tags.iter().any(|t| t == "evaluation") && !tags.iter().any(|t| t == "room") {
        tags.push("room".into());
        tags.sort();
    }
    tags
}

fn write_labels(path: &Path, labels: &Labels) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let body = serde_json::to_string(labels).map_err(|e| e.to_string())? + "\n";
    std::fs::write(path, body).map_err(|e| e.to_string())
}

pub fn set_starred(path: &Path, starred: bool) -> Result<Labels, String> {
    let mut labels = load(path);
    labels.starred = starred;
    write_labels(path, &labels)?;
    Ok(labels)
}

pub fn set_tags(path: &Path, tags: Vec<String>) -> Result<Labels, String> {
    let mut labels = load(path);
    labels.tags = normalize_tags(
        tags.into_iter()
            .map(|t| normalize_tag(&t))
            .collect::<Result<Vec<_>, _>>()?,
    );
    write_labels(path, &labels)?;
    Ok(labels)
}

pub fn add_tag(path: &Path, raw: &str) -> Result<Labels, String> {
    let tag = normalize_tag(raw)?;
    let mut labels = load(path);
    if !labels.tags.iter().any(|t| t == &tag) {
        labels.tags.push(tag);
        labels.tags = normalize_tags(labels.tags);
        write_labels(path, &labels)?;
    }
    Ok(labels)
}

pub fn remove_tag(path: &Path, raw: &str) -> Result<Labels, String> {
    let tag = normalize_tag(raw)?;
    let mut labels = load(path);
    labels.tags.retain(|t| t != &tag);
    write_labels(path, &labels)?;
    Ok(labels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluation_implies_room() {
        assert_eq!(
            normalize_tags(vec!["evaluation".into()]),
            vec!["evaluation", "room"]
        );
    }

    #[test]
    fn set_tags_round_trip() {
        let dir = std::env::temp_dir().join(format!("hive-labels-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("demo.labels");
        let labels = set_tags(&path, vec!["evaluation".into()]).unwrap();
        assert_eq!(labels.tags, vec!["evaluation", "room"]);
        assert!(load(&path).tags.contains(&"room".into()));
        let _ = std::fs::remove_dir_all(dir);
    }
}
