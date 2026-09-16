use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Labels {
    #[serde(default)]
    pub starred: bool,
    #[serde(default)]
    pub tags: Vec<serde_json::Value>,
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

pub fn set_starred(path: &Path, starred: bool) -> Result<Labels, String> {
    let mut labels = load(path);
    labels.starred = starred;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let body = serde_json::to_string(&labels).map_err(|e| e.to_string())? + "\n";
    std::fs::write(path, body).map_err(|e| e.to_string())?;
    Ok(labels)
}
