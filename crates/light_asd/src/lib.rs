//! Rust sidecar runner for the upstream Light-ASD Python demo and speaker attribution.
//!
//! This crate launches `Columbia_test.py` and active speaker attribution scripts;
//! it does not reimplement the model or its face/audio preprocessing. Python, the
//! upstream repository, PyTorch, model weights, and FFmpeg must be installed separately.

use anyhow::{Context, Result, bail};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub mod native_attribution;
pub use native_attribution::{
    CharacterVoiceProfile, Voiceprint, VoiceprintGallery, attribute_cues_dynamic,
    attribute_vtt_file_native, attribute_vtt_file_native_with_gallery, estimate_pitch_autocorr,
    evaluate_vtt_accuracy, extract_voiceprint, parse_character_from_caption, read_wav_samples,
};

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

/// Configuration for speaker attribution and WebVTT voice-tagging (<v Speaker>text</v>).
#[derive(Debug, Clone)]
pub struct SpeakerAttributionConfig {
    /// Python executable, for example `python` or a virtual environment interpreter.
    pub python: PathBuf,
    /// Speaker attribution script, defaults to `scripts/speaker_attribution.py`.
    pub script: PathBuf,
    /// Input video file.
    pub video: PathBuf,
    /// Input WebVTT file (merged or subtitle).
    pub input_vtt: PathBuf,
    /// Output WebVTT file with `<v Character>subtitle</v>` voice tags.
    pub output_vtt: PathBuf,
    /// Optional ground truth diarized WebVTT reference for evaluation.
    pub eval_ref: Option<PathBuf>,
    /// Whether to run the 100% Pure Rust attribution engine (default: true).
    pub use_native: bool,
    /// Optional persistent character voiceprint gallery JSON file.
    pub gallery_path: Option<PathBuf>,
}

impl SpeakerAttributionConfig {
    pub fn new(
        video: impl Into<PathBuf>,
        input_vtt: impl Into<PathBuf>,
        output_vtt: impl Into<PathBuf>,
    ) -> Self {
        let manifest_script = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("scripts")
            .join("speaker_attribution.py");
        let script = if manifest_script.is_file() {
            manifest_script
        } else if Path::new("crates/light_asd/scripts/speaker_attribution.py").is_file() {
            PathBuf::from("crates/light_asd/scripts/speaker_attribution.py")
        } else {
            PathBuf::from("scripts/speaker_attribution.py")
        };
        Self {
            python: PathBuf::from("python"),
            script,
            video: video.into(),
            input_vtt: input_vtt.into(),
            output_vtt: output_vtt.into(),
            eval_ref: None,
            use_native: true,
            gallery_path: None,
        }
    }

    pub fn with_gallery(mut self, gallery_path: impl Into<PathBuf>) -> Self {
        self.gallery_path = Some(gallery_path.into());
        self
    }

    pub fn with_eval_ref(mut self, eval_ref: impl Into<PathBuf>) -> Self {
        self.eval_ref = Some(eval_ref.into());
        self
    }

    pub fn with_python(mut self, python: impl Into<PathBuf>) -> Self {
        self.python = python.into();
        self.use_native = false;
        self
    }

    /// Executes speaker attribution and returns the output statistics.
    pub fn run(&self) -> Result<SpeakerAttributionResult> {
        if self.use_native {
            return self.run_native();
        }

        let python = resolve_python(&self.python)?;
        ensure_file(&self.script, "Speaker attribution script")?;
        ensure_file(&self.video, "Input video")?;
        ensure_file(&self.input_vtt, "Input WebVTT")?;
        if let Some(ref eval_ref) = self.eval_ref {
            ensure_file(eval_ref, "Reference diarized WebVTT")?;
        }

        let mut command = build_attribution_command(
            &python,
            &self.script,
            &self.video,
            &self.input_vtt,
            &self.output_vtt,
            self.eval_ref.as_deref(),
        );
        add_python_scripts_to_path(&mut command, &python)?;

        let output = command.output().with_context(|| {
            format!("failed to launch speaker attribution via {:?}", self.python)
        })?;
        let output = output_result(output, "Speaker attribution")?;
        let (total_subtitles, accuracy_percent) = parse_attribution_output(&output.stdout)?;

        Ok(SpeakerAttributionResult {
            output_vtt: self.output_vtt.clone(),
            total_subtitles,
            accuracy_percent,
            output,
        })
    }

    /// Executes 100% Pure-Rust speaker attribution without requiring Python or PyTorch.
    pub fn run_native(&self) -> Result<SpeakerAttributionResult> {
        ensure_file(&self.video, "Input video")?;
        ensure_file(&self.input_vtt, "Input WebVTT")?;
        if let Some(ref eval_ref) = self.eval_ref {
            ensure_file(eval_ref, "Reference diarized WebVTT")?;
        }

        let total_subtitles = attribute_vtt_file_native_with_gallery(
            &self.video,
            &self.input_vtt,
            &self.output_vtt,
            self.gallery_path.as_deref(),
        )?;

        let mut stdout = format!(
            "[ASD: DONE] Attributed {} subtitle cues -> {}\n",
            total_subtitles,
            self.output_vtt.display()
        );

        let accuracy_percent = if let Some(ref eval_ref) = self.eval_ref {
            let acc = evaluate_vtt_accuracy(&self.output_vtt, eval_ref)?;
            stdout.push_str(&format!(
                "[ASD: EVAL] Accuracy: {:.2}% against {}\n",
                acc,
                eval_ref.display()
            ));
            if acc >= 90.0 {
                stdout.push_str(&format!(
                    "[ASD: EVAL] SUCCESS: Accuracy {:.2}% >= 90.0% goal!\n",
                    acc
                ));
            } else {
                stdout.push_str(&format!(
                    "[ASD: EVAL] WARNING: Accuracy {:.2}% below 90.0% goal.\n",
                    acc
                ));
            }
            Some(acc)
        } else {
            None
        };

        Ok(SpeakerAttributionResult {
            output_vtt: self.output_vtt.clone(),
            total_subtitles,
            accuracy_percent,
            output: DemoOutput {
                stdout,
                stderr: String::new(),
                exit_code: Some(0),
            },
        })
    }
}

/// Output of a speaker attribution run.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeakerAttributionResult {
    pub output_vtt: PathBuf,
    pub total_subtitles: usize,
    pub accuracy_percent: Option<f32>,
    pub output: DemoOutput,
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

fn build_attribution_command(
    python: &Path,
    script: &Path,
    video: &Path,
    input_vtt: &Path,
    output_vtt: &Path,
    eval_ref: Option<&Path>,
) -> Command {
    let mut command = Command::new(python);
    command
        .arg(script)
        .arg("--video")
        .arg(video)
        .arg("--input-vtt")
        .arg(input_vtt)
        .arg("--output-vtt")
        .arg(output_vtt);
    if let Some(ref_path) = eval_ref {
        command.arg("--eval-ref").arg(ref_path);
    }
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

fn parse_attribution_output(stdout: &str) -> Result<(usize, Option<f32>)> {
    let mut total_subtitles = 0;
    let mut accuracy = None;

    for line in stdout.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("[ASD: DONE] Attributed ")
            && let Some(num_str) = rest.split_whitespace().next()
        {
            total_subtitles = num_str.parse::<usize>().unwrap_or(0);
        }
        if let Some(idx) = line.find("Accuracy: ") {
            let after = &line[idx + "Accuracy: ".len()..];
            if let Some(score_str) = after.split('%').next()
                && let Ok(score) = score_str.trim().parse::<f32>()
            {
                accuracy = Some(score);
            }
        }
    }
    Ok((total_subtitles, accuracy))
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
    fn speaker_attribution_command_receives_expected_arguments() {
        let command = build_attribution_command(
            Path::new("python"),
            Path::new("scripts/speaker_attribution.py"),
            Path::new("video.mp4"),
            Path::new("input.vtt"),
            Path::new("output.vtt"),
            Some(Path::new("ref.vtt")),
        );

        let args: Vec<_> = command.get_args().collect();
        assert_eq!(
            args,
            [
                OsStr::new("scripts/speaker_attribution.py"),
                OsStr::new("--video"),
                OsStr::new("video.mp4"),
                OsStr::new("--input-vtt"),
                OsStr::new("input.vtt"),
                OsStr::new("--output-vtt"),
                OsStr::new("output.vtt"),
                OsStr::new("--eval-ref"),
                OsStr::new("ref.vtt"),
            ]
        );
    }

    #[test]
    fn speaker_attribution_parser_extracts_count_and_accuracy() {
        let stdout = "[ASD: DONE] Attributed 64 subtitle cues -> out.vtt\n\
                      [ASD: EVAL] Aligned 61/61 cues | Accuracy: 96.72% (59/61) against ref.vtt\n";
        let (count, acc) = parse_attribution_output(stdout).unwrap();
        assert_eq!(count, 64);
        assert_eq!(acc, Some(96.72));
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
