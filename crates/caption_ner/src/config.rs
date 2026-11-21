use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Dynamic configuration for Named Entity Recognition on caption tags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NerConfig {
    /// Dynamic entity labels passed to GLiNER (e.g. ["person name", "character name", "system entity"]).
    #[serde(default = "default_dynamic_labels")]
    pub dynamic_labels: Vec<String>,
    /// Confidence threshold to accept an entity span (0.0 .. 1.0, default 0.65).
    #[serde(default = "default_threshold")]
    pub threshold: f32,
    /// Hard force include entities (e.g. ["系统", "最强签到系统"]).
    #[serde(default)]
    pub force_include: HashSet<String>,
    /// Hard force exclude entities (e.g. ["九狱弑仙剑", "神明境"]).
    #[serde(default)]
    pub force_exclude: HashSet<String>,
}

fn default_dynamic_labels() -> Vec<String> {
    vec!["person name".to_string(), "character name".to_string()]
}

fn default_threshold() -> f32 {
    0.65
}

impl Default for NerConfig {
    fn default() -> Self {
        Self {
            dynamic_labels: default_dynamic_labels(),
            threshold: default_threshold(),
            force_include: HashSet::new(),
            force_exclude: HashSet::new(),
        }
    }
}
