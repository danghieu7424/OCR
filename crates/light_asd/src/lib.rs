//! Rust sidecar runner for the upstream Light-ASD Python demo.
//!
//! This crate launches `Columbia_test.py`; it does not reimplement the model
//! or its face/audio preprocessing. Python, the upstream repository, PyTorch,
//! model weights, and FFmpeg must be installed separately.

use anyhow::{Context, Result, bail};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Paths and runtime configuration for a Light-ASD demo invocation.
#[derive(Debug, Clone)]
pub struct LightAsdConfig {
    /// Python executable, for example `python` or an environment-specific path.
    pub python: PathBuf,
    /// Root of the cloned `Junhua-Liao/Light-ASD` repository.
    pub repository: PathBuf,
    /// Input video file.
    pub video: PathBuf,
    /// Light-ASD checkpoint, usually `weight/finetuning_TalkSet.model`.
    pub model: PathBuf,
}

impl LightAsdConfig {
    pub fn new(
        repository: impl Into<PathBuf>,
        video: impl Into<PathBuf>,
        model: impl Into<PathBuf>,
    ) -> Self {
        Self {
            python: PathBuf::from("python"),
            repository: repository.into(),
            video: video.into(),
            model: model.into(),
        }
    }

    /// Runs the upstream demo and returns its captured output on success.
    pub fn run_demo(&self) -> Result<DemoOutput> {
        let repository = self.repository.canonicalize().with_context(|| {
            format!(
                "Light-ASD repository does not exist: {}",
                self.repository.display()
            )
        })?;
        let script = repository.join("Columbia_test.py");
        ensure_file(&script, "Light-ASD demo script")?;
        let model = resolve_model(&repository, &self.model)?;
        let video = self
            .video
            .canonicalize()
            .with_context(|| format!("input video does not exist: {}", self.video.display()))?;
        let video_name = video
            .file_stem()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .context("input video must have a valid file name")?;
        let video_folder = video
            .parent()
            .context("input video must have a parent directory")?;

        let python = resolve_python(&self.python)?;
        let mut command = build_command(&python, &script, video_name, video_folder, &model);
        command.current_dir(&repository);
        add_python_scripts_to_path(&mut command, &python)?;
        let output = command
            .output()
            .with_context(|| format!("failed to launch Python executable {:?}", self.python))?;

        output_result(output, "Light-ASD demo")
    }
}

/// Configuration for the upstream Columbia Active Speaker Detection benchmark.
#[derive(Debug, Clone)]
pub struct ColumbiaEvaluationConfig {
    /// Python executable, for example `python` or a virtual-environment path.
    pub python: PathBuf,
    /// Root of the cloned `Junhua-Liao/Light-ASD` repository.
    pub repository: PathBuf,
    /// Model checkpoint, relative to the repository or an absolute path.
    pub model: PathBuf,
    /// Directory used by the upstream script for downloaded data and outputs.
    pub data_directory: PathBuf,
}

impl ColumbiaEvaluationConfig {
    pub fn new(
        repository: impl Into<PathBuf>,
        model: impl Into<PathBuf>,
        data_directory: impl Into<PathBuf>,
    ) -> Self {
        Self {
            python: PathBuf::from("python"),
            repository: repository.into(),
            model: model.into(),
            data_directory: data_directory.into(),
        }
    }

    /// Downloads/runs the labeled Columbia benchmark and returns its average F1.
    pub fn run(&self) -> Result<ColumbiaEvaluationResult> {
        let repository = self.repository.canonicalize().with_context(|| {
            format!(
                "Light-ASD repository does not exist: {}",
                self.repository.display()
            )
        })?;
        let script = repository.join("Columbia_test.py");
        ensure_file(&script, "Light-ASD benchmark script")?;
        let model = resolve_model(&repository, &self.model)?;
        let data_directory = absolute_path(&self.data_directory)?;
        fs::create_dir_all(&data_directory).with_context(|| {
            format!(
                "cannot create Columbia data directory: {}",
                data_directory.display()
            )
        })?;
        let python = resolve_python(&self.python)?;

        let mut command = build_columbia_command(&python, &script, &model, &data_directory);
        command.current_dir(&repository);
        add_python_scripts_to_path(&mut command, &python)?;
        let output = command
            .output()
            .with_context(|| format!("failed to launch Python executable {:?}", self.python))?;
        let output = output_result(output, "Light-ASD Columbia evaluation")?;
        let average_f1_percent = parse_average_f1(&output.stdout)?;
        Ok(ColumbiaEvaluationResult {
            output,
            average_f1_percent,
        })
    }
}

/// Successful Columbia benchmark output and the reported average F1 percentage.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumbiaEvaluationResult {
    pub output: DemoOutput,
    pub average_f1_percent: f32,
}

/// Captured console output from a successful upstream demo run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

fn resolve_model(repository: &Path, model: &Path) -> Result<PathBuf> {
    let path = if model.is_absolute() {
        model.to_path_buf()
    } else {
        repository.join(model)
    };
    path.canonicalize()
        .with_context(|| format!("Light-ASD model does not exist: {}", path.display()))
}

fn resolve_python(python: &Path) -> Result<PathBuf> {
    if python.is_absolute() || python.components().count() > 1 {
        python
            .canonicalize()
            .with_context(|| format!("Python executable does not exist: {}", python.display()))
    } else {
        Ok(python.to_path_buf())
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()
            .context("cannot determine current directory")?
            .join(path))
    }
}

fn add_python_scripts_to_path(command: &mut Command, python: &Path) -> Result<()> {
    let mut paths = Vec::new();
    if let Some(parent) = python.parent().filter(|path| !path.as_os_str().is_empty()) {
        paths.push(parent.to_path_buf());
    }
    if let Some(path) = env::var_os("PATH") {
        paths.extend(env::split_paths(&path));
    }
    command.env(
        "PATH",
        env::join_paths(paths).context("cannot construct PATH for Python sidecar")?,
    );
    Ok(())
}

fn build_command(
    python: &Path,
    script: &Path,
    video_name: &str,
    video_folder: &Path,
    model: &Path,
) -> Command {
    let mut command = Command::new(python);
    command
        .arg(script)
        .arg("--videoName")
        .arg(video_name)
        .arg("--videoFolder")
        .arg(video_folder)
        .arg("--pretrainModel")
        .arg(model);
    command
}

fn build_columbia_command(
    python: &Path,
    script: &Path,
    model: &Path,
    data_directory: &Path,
) -> Command {
    let mut command = Command::new(python);
    command
        .arg(script)
        .arg("--evalCol")
        .arg("--pretrainModel")
        .arg(model)
        .arg("--colSavePath")
        .arg(data_directory);
    command
}

fn parse_average_f1(stdout: &str) -> Result<f32> {
    let score = stdout
        .lines()
        .find_map(|line| line.trim().strip_prefix("Average F1:"))
        .context("Light-ASD output did not contain the Columbia `Average F1` result")?
        .trim()
        .parse::<f32>()
        .context("Light-ASD Columbia average F1 was not a number")?;
    anyhow::ensure!(
        score.is_finite() && (0.0..=100.0).contains(&score),
        "Light-ASD Columbia average F1 must be in [0, 100], got {score}"
    );
    Ok(score)
}

fn output_result(output: Output, operation: &str) -> Result<DemoOutput> {
    if !output.status.success() {
        bail!(
            "{operation} failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(DemoOutput {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code(),
    })
}

fn ensure_file(path: &Path, description: &str) -> Result<()> {
    anyhow::ensure!(
        path.is_file(),
        "{description} not found: {}",
        path.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn light_asd_demo_receives_expected_arguments() {
        let command = build_command(
            Path::new("python"),
            Path::new("Columbia_test.py"),
            "episode",
            Path::new(r"C:\videos"),
            Path::new(r"C:\Light-ASD\weight\finetuning_TalkSet.model"),
        );

        let args: Vec<_> = command.get_args().collect();
        assert_eq!(
            args,
            [
                OsStr::new("Columbia_test.py"),
                OsStr::new("--videoName"),
                OsStr::new("episode"),
                OsStr::new("--videoFolder"),
                OsStr::new(r"C:\videos"),
                OsStr::new("--pretrainModel"),
                OsStr::new(r"C:\Light-ASD\weight\finetuning_TalkSet.model"),
            ]
        );
    }

    #[test]
    fn columbia_evaluation_uses_upstream_labeled_benchmark_arguments() {
        let command = build_columbia_command(
            Path::new("python"),
            Path::new("Columbia_test.py"),
            Path::new(r"C:\Light-ASD\weight\finetuning_TalkSet.model"),
            Path::new(r"C:\columbia"),
        );

        let args: Vec<_> = command.get_args().collect();
        assert_eq!(
            args,
            [
                OsStr::new("Columbia_test.py"),
                OsStr::new("--evalCol"),
                OsStr::new("--pretrainModel"),
                OsStr::new(r"C:\Light-ASD\weight\finetuning_TalkSet.model"),
                OsStr::new("--colSavePath"),
                OsStr::new(r"C:\columbia"),
            ]
        );
    }

    #[test]
    fn columbia_f1_parser_requires_a_finite_percentage() {
        assert_eq!(
            parse_average_f1("bell, ACC:96.2, F1:95.4\nAverage F1:95.50").unwrap(),
            95.5
        );
        assert!(parse_average_f1("No benchmark score").is_err());
        assert!(parse_average_f1("Average F1:NaN").is_err());
        assert!(parse_average_f1("Average F1:100.1").is_err());
    }
}
