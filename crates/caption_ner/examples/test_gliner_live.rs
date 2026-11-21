use anyhow::Result;
use caption_ner::{CaptionEntityExtractor, NerConfig};
use std::path::Path;

fn main() -> Result<()> {
    let extractor = CaptionEntityExtractor::from_model_dir(Path::new("models"))?;
    println!("GLiNER ONNX active: {}", extractor.is_onnx_active());

    let test_captions = [
        "《雪天妃》",
        "《陆清安-妖魔体状态》",
        "《大女儿-陆紫萱》",
        "《地底溶洞》",
        "《逃离虛空》",
        "《本体人形》",
        "《生成》",
        "《三位神明境強者》",
        "《九狱弑仙剑》",
        "《系统》",
    ];

    println!("\n=== RUN 1: Default (Labels: person, character) ===");
    let config1 = NerConfig::default();
    for cap in &test_captions {
        let extracted = extractor.extract(cap, &config1);
        println!("{:25} -> {:?}", cap, extracted);
    }

    println!("\n=== RUN 2: Added 'system' to dynamic_labels or force_include ===");
    let mut config2 = NerConfig::default();
    config2.dynamic_labels.push("system".to_string());
    config2.force_include.insert("系统".to_string());
    for cap in &test_captions {
        let extracted = extractor.extract(cap, &config2);
        println!("{:25} -> {:?}", cap, extracted);
    }

    Ok(())
}
