//! # Ollama Integration for Rust (Tự khởi động, tự setup, tự bảo đảm dữ liệu sử dụng)
//!
//! Module sử dụng thư viện chính thức `ollama-rs`:
//! - Tự động phát hiện đường dẫn binary `ollama` trên hệ điều hành.
//! - Tự động kích hoạt daemon `ollama serve` nếu server chưa chạy.
//! - Tự động kiểm tra và pull model (`qwen2.5vl:3b`) nếu máy chưa có.
//! - Cung cấp API trực tiếp cho việc hiệu đính phụ đề bằng Rust.

use anyhow::{bail, Context, Result};
use ollama_rs::{
    generation::chat::{request::ChatMessageRequest, ChatMessage},
    Ollama,
};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::glossary::EntityGlossary;
use crate::shared::terminal_color::NameUtilsExt;

/// Quản lý vòng đời và kết nối tới Ollama từ Rust
#[derive(Clone)]
pub struct OllamaManager {
    pub client: Ollama,
    pub model_name: String,
    pub host: String,
    pub port: u16,
    _daemon_process: Arc<Mutex<Option<Child>>>,
}

impl OllamaManager {
    /// Khởi tạo và tự động kiểm tra/setup toàn bộ môi trường (Daemon + Model)
    pub fn init_or_setup(model_name: &str) -> Result<Self> {
        let host = "http://127.0.0.1".to_string();
        let port = 11434u16;

        let manager = Self {
            client: Ollama::builder().host(host.clone()).port(port).build(),
            model_name: model_name.to_string(),
            host,
            port,
            _daemon_process: Arc::new(Mutex::new(None)),
        };

        // 1. Tự động kiểm tra và khởi động Ollama daemon nếu chưa chạy
        manager.ensure_daemon_running()?;

        // 2. Tự động kiểm tra và pull model nếu chưa có sẵn
        manager.ensure_model_available()?;

        Ok(manager)
    }

    /// Tìm đường dẫn binary `ollama` trên máy
    pub fn find_ollama_binary() -> Result<PathBuf> {
        // 1. Thử lệnh trong PATH
        if let Ok(output) = Command::new("where").arg("ollama").output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(first_line) = stdout.lines().next() {
                    let path = PathBuf::from(first_line.trim());
                    if path.exists() {
                        return Ok(path);
                    }
                }
            }
        }

        // 2. Thử các đường dẫn mặc định trên Windows
        if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
            let p = Path::new(&local_appdata).join("Programs/Ollama/ollama.exe");
            if p.exists() {
                return Ok(p);
            }
        }
        if let Ok(program_files) = std::env::var("ProgramFiles") {
            let p = Path::new(&program_files).join("Ollama/ollama.exe");
            if p.exists() {
                return Ok(p);
            }
        }

        bail!("Không tìm thấy binary `ollama` trên hệ thống. Vui lòng cài đặt từ https://ollama.com");
    }

    /// Kiểm tra cổng 11434, nếu chưa chạy thì tự khởi động `ollama serve` ngầm
    pub fn ensure_daemon_running(&self) -> Result<()> {
        if self.is_server_responsive() {
            crate::shared::tui_events::emit_log(format!(
                "   {} Ollama daemon đã sẵn sàng tại {}:{}",
                "[OLLAMA]".name_utils().violed(),
                self.host,
                self.port
            ));
            return Ok(());
        }

        crate::shared::tui_events::emit_log(format!(
            "   {} Phát hiện Ollama daemon chưa chạy. Đang tự động khởi động...",
            "[OLLAMA]".name_utils().violed()
        ));

        let bin = Self::find_ollama_binary()?;
        let child = Command::new(&bin)
            .arg("serve")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("Không thể khởi động `ollama serve` từ {:?}", bin))?;

        if let Ok(mut guard) = self._daemon_process.lock() {
            *guard = Some(child);
        }

        // Đợi tối đa 15 giây để server sẵn sàng
        let start = Instant::now();
        let timeout = Duration::from_secs(15);
        while start.elapsed() < timeout {
            std::thread::sleep(Duration::from_millis(600));
            if self.is_server_responsive() {
                crate::shared::tui_events::emit_log(format!(
                    "   {} Ollama daemon tự khởi động thành công!",
                    "[OLLAMA]".name_utils().violed()
                ));
                return Ok(());
            }
        }

        bail!("Ollama daemon khởi động quá thời gian chờ (15s) tại cổng 11434");
    }

    /// Kiểm tra phản hồi từ Ollama API
    fn is_server_responsive(&self) -> bool {
        let url = format!("{}:{}/api/tags", self.host, self.port);
        // Kiểm tra nhanh qua curl hoặc reqwest
        Command::new("curl.exe")
            .args(&["-s", "-f", "--max-time", "1", &url])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Tự động kiểm tra model, nếu chưa có thì tự động pull
    pub fn ensure_model_available(&self) -> Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        let models = rt.block_on(async { self.client.list_local_models().await })?;

        let has_model = models.iter().any(|m| {
            m.name == self.model_name
                || m.name.starts_with(&format!("{}:", self.model_name))
                || self.model_name.starts_with(&format!("{}:", m.name))
        });

        if has_model {
            crate::shared::tui_events::emit_log(format!(
                "   {} Mô hình `{}` đã sẵn sàng",
                "[OLLAMA]".name_utils().violed(),
                self.model_name
            ));
            return Ok(());
        }

        crate::shared::tui_events::emit_log(format!(
            "   {} Chưa có mô hình `{}`. Bắt đầu tự động tải (ollama pull)...",
            "[OLLAMA]".name_utils().violed(),
            self.model_name
        ));

        let bin = Self::find_ollama_binary()?;
        let status = Command::new(&bin)
            .args(&["pull", &self.model_name])
            .status()
            .with_context(|| format!("Lỗi khi tải mô hình {}", self.model_name))?;

        if !status.success() {
            bail!("Tải mô hình `{}` thất bại", self.model_name);
        }

        crate::shared::tui_events::emit_log(format!(
            "   {} Tải thành công mô hình `{}`!",
            "[OLLAMA]".name_utils().violed(),
            self.model_name
        ));
        Ok(())
    }

    /// Hiệu đính câu phụ đề bằng mô hình qua thư viện `ollama-rs`
    pub fn proofread_sentence_sync(
        &self,
        raw_text: &str,
        context_history: &[String],
        glossary: Option<&EntityGlossary>,
    ) -> Result<String> {
        let glossary_prompt = if let Some(g) = glossary {
            format!("Danh từ riêng chuẩn: {}\n", g.terms().join(", "))
        } else {
            String::new()
        };

        let context_prompt = if !context_history.is_empty() {
            let recent: Vec<&str> = context_history
                .iter()
                .rev()
                .take(3)
                .map(|s| s.as_str())
                .collect();
            format!("Ngữ cảnh trước đó: {}\n", recent.join(" | "))
        } else {
            String::new()
        };

        let system_msg = "Bạn là chuyên gia hiệu đính phụ đề hoạt hình Trung Quốc. \
Nhiệm vụ: Sửa lỗi OCR trong câu phụ đề dựa trên ngữ cảnh và danh từ riêng chuẩn. \
Quy tắc: \
1. Chỉ sửa khi rõ ràng là lỗi OCR (như nhầm 参 thành 爹, 涵 thành 嗯, 自已 thành 自己, hoặc thừa ký tự lạ). \
2. Giữ nguyên ngữ cảnh hội thoại tự nhiên, không bịa thêm chữ. \
3. Chỉ xuất ra duy nhất câu phụ đề tiếng Trung đã sửa, không giải thích.";

        let user_msg = format!(
            "{}{}\nCâu phụ đề cần sửa: \"{}\"",
            glossary_prompt, context_prompt, raw_text
        );

        let messages = vec![
            ChatMessage::system(system_msg.to_string()),
            ChatMessage::user(user_msg),
        ];

        let req = ChatMessageRequest::new(self.model_name.clone(), messages);

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        let resp = rt.block_on(async { self.client.send_chat_messages(req).await })?;

        let text = resp.message.content.trim().trim_matches('"').to_string();
        Ok(text)
    }
}
