//! Rust sidecar runner for the upstream Light-ASD Python demo.
//!
//! This crate launches `Columbia_test.py`; it does not reimplement the model
//! or its face/audio preprocessing. Python, the upstream repository, PyTorch,
//! model weights, and FFmpeg must be installed separately.

use anyhow::{Context, Result, bail};
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
        let model = if self.model.is_absolute() {
            self.model.clone()
        } else {
            repository.join(&self.model)
        };
        let model = model
            .canonicalize()
            .with_context(|| format!("Light-ASD model does not exist: {}", model.display()))?;
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

        let output = build_command(&self.python, &script, video_name, video_folder, &model)
            .current_dir(&repository)
            .output()
            .with_context(|| format!("failed to launch Python executable {:?}", self.python))?;

        output_result(output)
    }
}

/// Captured console output from a successful upstream demo run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
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

fn output_result(output: Output) -> Result<DemoOutput> {
    if !output.status.success() {
        bail!(
            "Light-ASD demo failed with status {}: {}",
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
}
