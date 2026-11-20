use anyhow::{Context, Result, bail};
use light_asd::ColumbiaEvaluationConfig;
use std::env;
use std::path::PathBuf;

fn main() -> Result<()> {
    let mut args = env::args_os().skip(1);
    let repository = args
        .next()
        .map(PathBuf::from)
        .context("usage: evaluate_columbia <repo> <model> <data-dir> [python]")?;
    let model = args
        .next()
        .map(PathBuf::from)
        .context("usage: evaluate_columbia <repo> <model> <data-dir> [python]")?;
    let data_directory = args
        .next()
        .map(PathBuf::from)
        .context("usage: evaluate_columbia <repo> <model> <data-dir> [python]")?;
    let python = args.next().map(PathBuf::from);
    if args.next().is_some() {
        bail!("usage: evaluate_columbia <repo> <model> <data-dir> [python]");
    }

    let mut config = ColumbiaEvaluationConfig::new(repository, model, data_directory);
    if let Some(python) = python {
        config.python = python;
    }
    let result = config.run()?;
    print!("{}", result.output.stdout);
    eprint!("{}", result.output.stderr);
    println!("Average F1: {:.2}%", result.average_f1_percent);
    Ok(())
}
