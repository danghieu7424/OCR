use anyhow::Result;
use light_asd::evaluate_vtt_accuracy;
use std::env;
use std::path::Path;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: evaluate_attribution <ref_vtt> <attr_vtt>");
        std::process::exit(1);
    }
    let ref_path = Path::new(&args[1]);
    let attr_path = Path::new(&args[2]);
    let acc = evaluate_vtt_accuracy(attr_path, ref_path)?;
    println!(
        "Evaluation: {} vs {}",
        ref_path.display(),
        attr_path.display()
    );
    println!("Speaker Attribution Accuracy: {:.2}%", acc);
    Ok(())
}
