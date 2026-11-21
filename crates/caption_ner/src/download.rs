use anyhow::{Context, Result, bail};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const HF_PRIMARY_URL: &str = "https://huggingface.co/onnx-community/gliner_multi-v2.1/resolve/main";
const HF_MIRROR_URL: &str = "https://hf-mirror.com/onnx-community/gliner_multi-v2.1/resolve/main";

const MODEL_QUANTIZED_FILE: &str = "model_quantized.onnx";
const TOKENIZER_FILE: &str = "tokenizer.json";
const CONFIG_FILE: &str = "gliner_config.json";

/// Downloads a single file from primary URL or fallback mirror with progress tracking.
pub fn download_file_with_progress(
    filename: &str,
    subpath: &str,
    dest_dir: &Path,
    min_expected_bytes: u64,
) -> Result<PathBuf> {
    fs::create_dir_all(dest_dir)
        .with_context(|| format!("cannot create directory: {}", dest_dir.display()))?;

    let dest_path = dest_dir.join(filename);
    if dest_path.is_file()
        && let Ok(meta) = fs::metadata(&dest_path)
        && meta.len() >= min_expected_bytes
    {
        return Ok(dest_path);
    }

    let primary_url = format!("{HF_PRIMARY_URL}/{subpath}");
    let mirror_url = format!("{HF_MIRROR_URL}/{subpath}");

    let urls = [&primary_url, &mirror_url];
    let mut last_err = None;

    let tmp_path = dest_dir.join(format!("{filename}.downloading"));
    let _ = fs::remove_file(&tmp_path);

    for url in urls {
        eprintln!("[GLiNER: DL] Fetching {filename} from {url} ...");
        let resp = match ureq::get(url).call() {
            Ok(r) => r,
            Err(e) => {
                last_err = Some(anyhow::anyhow!("failed connecting to {url}: {e}"));
                continue;
            }
        };

        let total_size = resp
            .header("Content-Length")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        let pb = if total_size > 0 {
            let pb = ProgressBar::new(total_size);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template(" [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta}) {msg}")
                    .unwrap()
                    .progress_chars("▰▰▱"),
            );
            pb.set_message(format!("Downloading {filename}"));
            Some(pb)
        } else {
            None
        };

        let mut reader = resp.into_reader();
        let mut file = match File::create(&tmp_path) {
            Ok(f) => f,
            Err(e) => {
                last_err = Some(anyhow::anyhow!("cannot create tmp file: {e}"));
                continue;
            }
        };

        let mut buffer = [0u8; 64 * 1024];
        let mut downloaded = 0u64;
        let mut success = true;

        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    if let Err(e) = file.write_all(&buffer[..n]) {
                        last_err = Some(anyhow::anyhow!("failed writing file: {e}"));
                        success = false;
                        break;
                    }
                    downloaded += n as u64;
                    if let Some(ref pb) = pb {
                        pb.set_position(downloaded);
                    }
                }
                Err(e) => {
                    last_err = Some(anyhow::anyhow!("stream error while downloading: {e}"));
                    success = false;
                    break;
                }
            }
        }

        if let Some(ref pb) = pb {
            pb.finish_with_message(format!("Downloaded {filename}"));
        }

        if success && downloaded >= min_expected_bytes {
            fs::rename(&tmp_path, &dest_path)
                .with_context(|| format!("cannot rename to {}", dest_path.display()))?;
            eprintln!(
                "[GLiNER: DL: DONE] {filename} ready -> {}",
                dest_path.display()
            );
            return Ok(dest_path);
        } else {
            let _ = fs::remove_file(&tmp_path);
        }
    }

    if let Some(e) = last_err {
        bail!("Failed downloading {filename} from both sources: {e}");
    }
    bail!("Failed downloading {filename}: no sources succeeded");
}

/// Checks and downloads the complete GLiNER multilingual model files into `models_dir/gliner/`.
pub fn ensure_gliner_model_downloaded(models_dir: &Path) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let gliner_dir = models_dir.join("gliner");

    let model_path = download_file_with_progress(
        MODEL_QUANTIZED_FILE,
        &format!("onnx/{MODEL_QUANTIZED_FILE}"),
        &gliner_dir,
        100_000_000, // ~349 MB
    )?;

    let tokenizer_path = download_file_with_progress(
        TOKENIZER_FILE,
        TOKENIZER_FILE,
        &gliner_dir,
        5_000_000, // ~16 MB
    )?;

    let config_path = download_file_with_progress(
        CONFIG_FILE,
        CONFIG_FILE,
        &gliner_dir,
        100, // Small JSON
    )?;

    Ok((model_path, tokenizer_path, config_path))
}
