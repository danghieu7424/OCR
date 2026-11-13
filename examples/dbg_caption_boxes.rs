//! TEMP diagnostic: print DBNet boxes + ink colour stats for caption frames.
use anyhow::Result;
use std::path::Path;
use OCR::ocr::det_filter::create_det_engine;

fn main() -> Result<()> {
    let engine = create_det_engine("models/ocr/det.onnx", "models/ocr/rec.onnx", "models/ocr/keys.txt")?;
    for arg in std::env::args().skip(1) {
        let img = image::open(Path::new(&arg))?.to_rgb8();
        let (w, h) = (img.width() as usize, img.height() as usize);
        // ROI: full width, Y 10%-80%
        let crop_y = ((h as f32 * 0.10).round() as u32 / 2) * 2;
        let crop_w = w as u32 & !1;
        let crop_h = (((h as f32 * 0.70).round() as u32) / 2) * 2;
        let roi = image::imageops::crop_imm(&img, 0, crop_y, crop_w, crop_h).to_image();
        let dynimg = image::DynamicImage::ImageRgb8(roi.clone());
        let results = engine.run_from_image(&dynimg)?;
        println!("== {} ROI {}x{} -> {} boxes", arg, crop_w, crop_h, results.len());
        for r in &results {
            let pts: Vec<(f64, f64)> = r.bounding_box.exterior().points().map(|p| (p.x(), p.y())).collect();
            let (mut mnx, mut mny, mut mxx, mut mxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            for (x, y) in pts { mnx = mnx.min(x); mny = mny.min(y); mxx = mxx.max(x); mxy = mxy.max(y); }
            let (bw, bh) = (mxx - mnx, mxy - mny);
            // ink stats inside box
            let x0 = mnx.max(0.0) as u32;
            let y0 = mny.max(0.0) as u32;
            let x1 = (mxx as u32).min(roi.width());
            let y1 = (mxy as u32).min(roi.height());
            let mut lums: Vec<(f32, f32)> = Vec::new();
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = roi.get_pixel(x, y).0;
                    let (rf, gf, bf) = (p[0] as f32, p[1] as f32, p[2] as f32);
                    let mx = rf.max(gf).max(bf);
                    let mnn = rf.min(gf).min(bf);
                    let sat = if mx > 0.0 { (mx - mnn) / mx } else { 0.0 };
                    let lum = 0.299 * rf + 0.587 * gf + 0.114 * bf;
                    lums.push((lum, sat));
                }
            }
            lums.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
            let topn = (lums.len() / 4).max(1);
            let top_sat: Vec<f32> = lums[..topn].iter().map(|(_, s)| *s).collect();
            let med_sat = {
                let mut v = top_sat.clone();
                v.sort_by(|a, b| a.partial_cmp(b).unwrap());
                v[v.len() / 2]
            };
            let mean_lum_top: f32 = lums[..topn].iter().map(|(l, _)| *l).sum::<f32>() / topn as f32;
            println!("   box x={:.0} y={:.0} w={:.0} h={:.0} conf={:.2} topSat={:.2} topLum={:.0} text={:?}",
                mnx, mny, bw, bh, r.confidence, med_sat, mean_lum_top, r.text);
        }
    }
    Ok(())
}
