// src/bin/dump_patch.rs
use anyhow::Result;
use std::process::{Command, Stdio};
use std::io::Read;
use std::path::Path;

#[path = "../ocr/det_filter.rs"]
mod det_filter;

fn main() -> Result<()> {
    let model_dir = Path::new("models");
    let det_path = model_dir.join("ocr/det.onnx");
    let rec_path = model_dir.join("ocr/rec.onnx");
    let keys_path = model_dir.join("ocr/keys.txt");
    
    let det_engine = det_filter::create_det_engine(
        &det_path.to_string_lossy(),
        &rec_path.to_string_lossy(),
        &keys_path.to_string_lossy(),
    )?;

    if let Ok(img) = image::open("mi_frame.png") {
        let res = det_engine.run_from_image(&img)?;
        println!("mi_frame.png: {} boxes", res.len());
        for (bi, r) in res.iter().enumerate() {
            let pts: Vec<_> = r.bounding_box.exterior().points().map(|p| (p.x() as i32, p.y() as i32)).collect();
            println!("   Box {}: {:?}", bi, pts);
        }
    }
    Ok(())
}

