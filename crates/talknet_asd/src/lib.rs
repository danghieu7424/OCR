//! Rust sidecar runner for the upstream TalkNet-ASD Python demo.
//!
//! This crate launches `demoTalkNet.py`; it does not reimplement the model or
//! its face/audio preprocessing. Python, the upstream repository, PyTorch, and
//! FFmpeg must be installed separately.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Paths and runtime configuration for a TalkNet-ASD demo invocation.
#[derive(Debug, Clone)]
pub struct TalkNetConfig {
    /// Python executable, for example `python` or an environment-specific path.
    pub python: PathBuf,
    /// Root of the cloned `TaoRuijie/TalkNet-ASD` repository.
    pub repository: PathBuf,
    /// Input video file.
    pub video: PathBuf,
}

impl TalkNetConfig {
    pub fn new(repository: impl Into<PathBuf>, video: impl Into<PathBuf>) -> Self {
        Self {
            python: PathBuf::from("python"),
            repository: repository.into(),
            video: video.into(),
        }
    }

    /// Runs the upstream demo and returns its captured output on success.
    pub fn run_demo(&self) -> Result<DemoOutput> {
        let repository = self.repository.canonicalize().with_context(|| {
            format!(
                "TalkNet-ASD repository does not exist: {}",
                self.repository.display()
            )
        })?;
        let script = repository.join("demoTalkNet.py");
        ensure_file(&script, "TalkNet-ASD demo script")?;

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

        let output = build_command(&self.python, &script, video_name, video_folder)
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

fn build_command(python: &Path, script: &Path, video_name: &str, video_folder: &Path) -> Command {
    let mut command = Command::new(python);
    command
        .arg(script)
        .arg("--videoName")
        .arg(video_name)
        .arg("--videoFolder")
        .arg(video_folder);
    command
}

fn output_result(output: Output) -> Result<DemoOutput> {
    if !output.status.success() {
        bail!(
            "TalkNet-ASD demo failed with status {}: {}",
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
    fn talknet_demo_receives_video_stem_and_parent_folder() {
        let command = build_command(
            Path::new("python"),
            Path::new("demoTalkNet.py"),
            "episode",
            Path::new(r"C:\videos"),
        );

        let args: Vec<_> = command.get_args().collect();
        assert_eq!(
            args,
            [
                OsStr::new("demoTalkNet.py"),
                OsStr::new("--videoName"),
                OsStr::new("episode"),
                OsStr::new("--videoFolder"),
                OsStr::new(r"C:\videos"),
            ]
        );
    }
}
