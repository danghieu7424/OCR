use anyhow::Result;
use caption_ner::ensure_gliner_model_downloaded;
use std::path::Path;

fn main() -> Result<()> {
    println!("=== Auto-downloading GLiNER Multilingual ONNX (model_quantized.onnx ~349MB) ===");
    let models_dir = Path::new("models");
    let (model_path, tokenizer_path, config_path) = ensure_gliner_model_downloaded(models_dir)?;
    println!("GLiNER model ready at: {}", model_path.display());
    println!("Tokenizer ready at:    {}", tokenizer_path.display());
    println!("Config ready at:       {}", config_path.display());
    Ok(())
}
