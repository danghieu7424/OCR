// crates/ocr_shared/src/lib.rs
use std::sync::{Arc, Mutex, OnceLock};

pub struct CpuOptimizer;
impl CpuOptimizer {
    pub fn single_task_threads() -> usize {
        2
    }
}

pub mod terminal_color {
    pub struct NameWrapper(pub String);
    impl NameWrapper {
        pub fn violed(self) -> String {
            self.0
        }
    }

    pub trait NameUtilsExt {
        fn name_utils(&self) -> NameWrapper;
    }

    impl NameUtilsExt for str {
        fn name_utils(&self) -> NameWrapper {
            NameWrapper(self.to_string())
        }
    }

    impl NameUtilsExt for &str {
        fn name_utils(&self) -> NameWrapper {
            NameWrapper((*self).to_string())
        }
    }

    impl NameUtilsExt for String {
        fn name_utils(&self) -> NameWrapper {
            NameWrapper(self.clone())
        }
    }

    pub trait TermColorExt {
        fn violed(&self) -> String;
    }

    impl TermColorExt for str {
        fn violed(&self) -> String {
            self.to_string()
        }
    }

    impl TermColorExt for &str {
        fn violed(&self) -> String {
            (*self).to_string()
        }
    }

    impl TermColorExt for String {
        fn violed(&self) -> String {
            self.clone()
        }
    }
}

pub mod icons {
    pub struct Icons;
    impl Icons {
        pub const SUCCESS: &'static str = "✔ ";
    }
}

pub mod tui_events {
    use super::*;

    /// Dữ liệu chi tiết về tiến trình xử lý khung hình OCR
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct ProgressPayload {
        pub title: String,
        pub current: usize,
        pub total: usize,
        pub unit: String,
        pub elapsed: String,
        pub detail: String,
    }

    pub type StepHandler = Box<dyn Fn(&str) + Send + Sync + 'static>;
    pub type LogHandler = Box<dyn Fn(&str) + Send + Sync + 'static>;
    pub type ProgressHandler = Box<dyn Fn(ProgressPayload) + Send + Sync + 'static>;

    struct EventHandlerRegistry {
        step_handlers: Vec<StepHandler>,
        log_handlers: Vec<LogHandler>,
        progress_handlers: Vec<ProgressHandler>,
    }

    static REGISTRY: OnceLock<Mutex<EventHandlerRegistry>> = OnceLock::new();

    fn get_registry() -> &'static Mutex<EventHandlerRegistry> {
        REGISTRY.get_or_init(|| {
            Mutex::new(EventHandlerRegistry {
                step_handlers: Vec::new(),
                log_handlers: Vec::new(),
                progress_handlers: Vec::new(),
            })
        })
    }

    /// Đăng ký callback nhận sự kiện bước xử lý mới (emit_step)
    pub fn on_step<F>(handler: F)
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        if let Ok(mut reg) = get_registry().lock() {
            reg.step_handlers.push(Box::new(handler));
        }
    }

    /// Đăng ký callback nhận sự kiện log chi tiết (emit_log)
    pub fn on_log<F>(handler: F)
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        if let Ok(mut reg) = get_registry().lock() {
            reg.log_handlers.push(Box::new(handler));
        }
    }

    /// Đăng ký callback nhận sự kiện cập nhật tiến trình (emit_task_progress)
    pub fn on_progress<F>(handler: F)
    where
        F: Fn(ProgressPayload) + Send + Sync + 'static,
    {
        if let Ok(mut reg) = get_registry().lock() {
            reg.progress_handlers.push(Box::new(handler));
        }
    }

    /// Xóa sạch các callback đã đăng ký (dùng khi reset hoặc chuyển task)
    pub fn clear_handlers() {
        if let Ok(mut reg) = get_registry().lock() {
            reg.step_handlers.clear();
            reg.log_handlers.clear();
            reg.progress_handlers.clear();
        }
    }

    /// Cắm trực tiếp vào `TaskWidget` của `cli_gui_form` để đồng bộ tiến trình TUI!
    pub fn attach_cli_gui_task(task_widget: Arc<Mutex<cli_gui_form::TaskWidget>>) {
        on_progress(move |p| {
            if let Ok(mut w) = task_widget.lock() {
                match &mut w.state {
                    cli_gui_form::TaskState::Running {
                        current,
                        total,
                        duration,
                        status,
                        ..
                    } => {
                        *current = p.current.min(*total);
                        *duration = p.elapsed;
                        *status = p.detail;
                    }
                    _ => {
                        w.switch_to_progress(&p.title, p.total, &p.unit, &p.elapsed, &p.detail);
                    }
                }
            }
        });
    }

    /// Được gọi từ OCR engine khi bắt đầu một bước xử lý mới
    pub fn emit_step(msg: &str) {
        if let Ok(reg) = get_registry().lock() {
            for handler in &reg.step_handlers {
                handler(msg);
            }
        }
    }

    /// Được gọi từ OCR engine khi có log mới
    pub fn emit_log(msg: impl Into<String>) {
        let msg = msg.into();
        if let Ok(reg) = get_registry().lock() {
            for handler in &reg.log_handlers {
                handler(&msg);
            }
        }
    }

    /// Được gọi từ OCR engine để phát tín hiệu tiến trình khung hình
    pub fn emit_task_progress(
        title: &str,
        curr: usize,
        total: usize,
        unit: &str,
        elapsed: impl Into<String>,
        detail: impl Into<String>,
    ) {
        let payload = ProgressPayload {
            title: title.to_string(),
            current: curr,
            total,
            unit: unit.to_string(),
            elapsed: elapsed.into(),
            detail: detail.into(),
        };
        if let Ok(reg) = get_registry().lock() {
            for handler in &reg.progress_handlers {
                handler(payload.clone());
            }
        }
    }
}
