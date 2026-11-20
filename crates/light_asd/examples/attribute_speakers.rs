use anyhow::{Context, Result, bail};
use light_asd::SpeakerAttributionConfig;
use std::env;
use std::path::PathBuf;

fn main() -> Result<()> {
    let mut args = env::args_os().skip(1);
    let video = args.next().map(PathBuf::from).context(
        "usage: attribute_speakers <video> <input-vtt> <output-vtt> [eval-ref] [python]",
    )?;
    let input_vtt = args.next().map(PathBuf::from).context(
        "usage: attribute_speakers <video> <input-vtt> <output-vtt> [eval-ref] [python]",
    )?;
    let output_vtt = args.next().map(PathBuf::from).context(
        "usage: attribute_speakers <video> <input-vtt> <output-vtt> [eval-ref] [python]",
    )?;
    let eval_ref = args.next().map(PathBuf::from);
    let python = args.next().map(PathBuf::from);

    if args.next().is_some() {
        bail!("usage: attribute_speakers <video> <input-vtt> <output-vtt> [eval-ref] [python]");
    }

    let mut config = SpeakerAttributionConfig::new(video, input_vtt, output_vtt);
    if let Some(r) = eval_ref {
        config = config.with_eval_ref(r);
    }
    if let Some(p) = python {
        config.python = p;
    }

    let result = config.run()?;
    print!("{}", result.output.stdout);
    if !result.output.stderr.is_empty() {
        eprint!("{}", result.output.stderr);
    }
    println!("Output: {}", result.output_vtt.display());
    println!("Attributed subtitles: {}", result.total_subtitles);
    if let Some(acc) = result.accuracy_percent {
        println!("Accuracy: {:.2}%", acc);
    }
    Ok(())
}
