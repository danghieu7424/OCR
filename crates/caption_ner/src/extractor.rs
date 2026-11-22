use crate::config::NerConfig;
use crate::heuristic::heuristic_parse_character;
use anyhow::{Context, Result};
use gliner::model::pipeline::span::SpanMode;
use gliner::model::{GLiNER, input::text::TextInput, params::Parameters};
use orp::params::RuntimeParameters;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// Hybrid Character & Entity Extractor combining User Overrides, GLiNER Zero-Shot NER, and Heuristic Fallbacks.
pub struct CaptionEntityExtractor {
    model: Option<Arc<GLiNER<SpanMode>>>,
    memo_cache: RwLock<HashMap<String, Option<String>>>,
}

impl CaptionEntityExtractor {
    /// Creates an extractor by attempting to load GLiNER ONNX model from the specified directory.
    /// If the model is not found or GLINER_USE_ONNX is not enabled, gracefully falls back to Heuristic + User Overrides mode.
    pub fn from_model_dir(model_dir: &Path) -> Result<Self> {
        if std::env::var("GLINER_USE_ONNX").as_deref() == Ok("1") {
            let gliner_dir = model_dir.join("gliner");
            let model_quantized = gliner_dir.join("model_quantized.onnx");
            let model_full = gliner_dir.join("model.onnx");
            let tokenizer_path = gliner_dir.join("tokenizer.json");

            let onnx_path = if model_quantized.is_file() {
                Some(model_quantized)
            } else if model_full.is_file() {
                Some(model_full)
            } else {
                None
            };

            if let (Some(model_path), true) = (onnx_path, tokenizer_path.is_file()) {
                let params = Parameters::default();
                let runtime_params = RuntimeParameters::default();

                match GLiNER::<SpanMode>::new(
                    params,
                    runtime_params,
                    tokenizer_path.to_str().context("invalid tokenizer path")?,
                    model_path.to_str().context("invalid model path")?,
                ) {
                    Ok(engine) => {
                        eprintln!(
                            "[GLiNER: INIT] Zero-Shot NER model loaded successfully from {}",
                            gliner_dir.display()
                        );
                        return Ok(Self {
                            model: Some(Arc::new(engine)),
                            memo_cache: RwLock::new(HashMap::new()),
                        });
                    }
                    Err(e) => {
                        eprintln!(
                            "[GLiNER: WARN] Failed initializing ONNX engine ({e}). Falling back to heuristic mode."
                        );
                    }
                }
            }
        }

        Ok(Self::new_heuristic_only())
    }

    /// Creates an extractor in pure heuristic + rule mode without loading ONNX models.
    pub fn new_heuristic_only() -> Self {
        Self {
            model: None,
            memo_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Returns true if the zero-shot ONNX model is actively loaded.
    pub fn is_onnx_active(&self) -> bool {
        self.model.is_some()
    }

    /// Extracts a character/entity name from raw caption text using the Hybrid 2-tier pipeline.
    pub fn extract(&self, raw_vtt_line: &str, config: &NerConfig) -> Option<String> {
        let trimmed = raw_vtt_line.trim();
        let content = if let Some(stripped) = trimmed.strip_prefix('《') {
            stripped.strip_suffix('》').unwrap_or(stripped).trim()
        } else {
            trimmed
        };

        if content.is_empty() {
            return None;
        }

        // TIER 1: User Overrides (Zero cost, 100% Deterministic)
        // 1. Force Exclude
        if config.force_exclude.contains(content)
            || config.force_exclude.iter().any(|ex| content == ex)
        {
            return None;
        }

        // 2. Force Include (e.g. user added "系统" or "魔皇-斩天")
        if config.force_include.contains(content) {
            return Some(content.to_string());
        }
        for inc in &config.force_include {
            if content.contains(inc) {
                return Some(inc.clone());
            }
        }

        // 3. Memoization Cache Check
        let cache_key = format!(
            "{}|{:?}|{}",
            content, config.dynamic_labels, config.threshold
        );
        if let Ok(cache) = self.memo_cache.read()
            && let Some(cached_res) = cache.get(&cache_key)
        {
            return cached_res.clone();
        }

        // TIER 2: GLiNER Zero-Shot NER (if ONNX engine is active)
        let mut extracted_name = None;

        if let Some(ref engine) = self.model
            && !config.dynamic_labels.is_empty()
        {
            let texts = [content];
            let label_strs: Vec<&str> = config.dynamic_labels.iter().map(|s| s.as_str()).collect();

            if let Ok(input) = TextInput::from_str(&texts, &label_strs)
                && let Ok(output) = engine.inference(input)
                && let Some(spans) = output.spans.first()
            {
                let mut best_score = config.threshold;
                let mut best_span = None;

                for span in spans {
                    if span.probability() >= best_score {
                        best_score = span.probability();
                        best_span = Some(span.text().trim().to_string());
                    }
                }

                if let Some(s) = best_span
                    && !s.is_empty()
                {
                    extracted_name = Some(s);
                }
            }
        }

        // TIER 3: Advanced Heuristic Fallback
        if extracted_name.is_none() {
            extracted_name = heuristic_parse_character(content);
        }

        // Store result in cache
        if let Ok(mut cache) = self.memo_cache.write() {
            cache.insert(cache_key, extracted_name.clone());
        }

        extracted_name
    }

    /// Extracts all distinct characters/entities found across potentially multi-line or multi-tag captions.
    pub fn extract_all(&self, raw_vtt_text: &str, config: &NerConfig) -> Vec<String> {
        let mut results = Vec::new();
        for line in raw_vtt_text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let mut start_idx = 0;
            let mut found_brackets = false;
            while let Some(open) = trimmed[start_idx..].find('《') {
                let abs_open = start_idx + open;
                if let Some(close) = trimmed[abs_open..].find('》') {
                    let close_end = abs_open + close + '》'.len_utf8();
                    let tag = &trimmed[abs_open..close_end];
                    if let Some(ch) = self.extract(tag, config) {
                        if !results.contains(&ch) {
                            results.push(ch);
                        }
                    }
                    start_idx = close_end;
                    found_brackets = true;
                } else {
                    break;
                }
            }
            if !found_brackets {
                if let Some(ch) = self.extract(trimmed, config) {
                    if !results.contains(&ch) {
                        results.push(ch);
                    }
                }
            }
        }
        results
    }
}
