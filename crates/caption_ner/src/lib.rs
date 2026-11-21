pub mod config;
pub mod download;
pub mod extractor;
pub mod heuristic;

pub use config::NerConfig;
pub use download::ensure_gliner_model_downloaded;
pub use extractor::CaptionEntityExtractor;
pub use heuristic::heuristic_parse_character;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heuristic_extraction() {
        assert_eq!(
            heuristic_parse_character("《陆清安》"),
            Some("陆清安".to_string())
        );
        assert_eq!(
            heuristic_parse_character("《大女儿-陆紫萱》"),
            Some("陆紫萱".to_string())
        );
        assert_eq!(
            heuristic_parse_character("《陆清安-妖魔体状态》"),
            Some("陆清安".to_string())
        );
        assert_eq!(heuristic_parse_character("《三位神明境強者》"), None);
        assert_eq!(heuristic_parse_character("《九狱弑仙剑》"), None);
    }

    #[test]
    fn test_hybrid_override_pipeline() {
        let extractor = CaptionEntityExtractor::new_heuristic_only();

        // Run 1: Default config (does not extract "系统" because it's not a person)
        let config_run1 = NerConfig::default();
        let res1 = extractor.extract("《系统》", &config_run1);
        assert_eq!(res1, None);

        // Run 2: Dynamic Adaptation - User adds "系统" to force_include
        let mut config_run2 = NerConfig::default();
        config_run2.force_include.insert("系统".to_string());
        let res2 = extractor.extract("《系统》", &config_run2);
        assert_eq!(res2, Some("系统".to_string()));

        // Run 3: User excludes a character that would normally be accepted
        let mut config_run3 = NerConfig::default();
        config_run3.force_exclude.insert("陆清安".to_string());
        let res3 = extractor.extract("《陆清安》", &config_run3);
        assert_eq!(res3, None);
    }
}
