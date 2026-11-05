use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/****
 * Module: Tags
 * Chức năng: Hệ thống chuẩn hóa tiền tố cố định (Fixed-Width Tag System) cho TUI.
 *            - Chuẩn Text-based 4 ký tự: [XXXX] (Độ rộng chính xác: 6 cell).
 *            - Chuẩn Symbol-based 1 ký tự: [X] (Độ rộng chính xác: 3 cell).
 * Ranh giới bảo vệ: Triệt tiêu 100% Layout Shift (Jitter) khi render streaming logs,
 *            status bar, compact tree view, đồng thời cung cấp zero-allocation fallback.
 ****/
pub struct Tags;

impl Tags {
    // =========================================================================
    // 1. CHUẨN TEXT-BASED 4 KÝ TỰ: [XXXX] (TỔNG ĐỘ RỘNG CỐ ĐỊNH: 6 CELL)
    // =========================================================================

    // Trạng thái & Sự kiện (Status & Severity)
    pub const INFO: &'static str = "[INFO]"; // Thông tin điều hướng chung
    pub const WARN: &'static str = "[WARN]"; // Cảnh báo nghiệp vụ / tài nguyên
    pub const FAIL: &'static str = "[FAIL]"; // Thất bại / Lỗi ngoại lệ
    pub const PASS: &'static str = "[PASS]"; // Hợp lệ / Kiểm thử thành công
    pub const DONE: &'static str = "[DONE]"; // Hoàn tất công việc / tác vụ
    pub const WAIT: &'static str = "[WAIT]"; // Đang xếp hàng / Chờ tài nguyên
    pub const IDLE: &'static str = "[IDLE]"; // Trạng thái nghỉ / Sẵn sàng
    pub const BUSY: &'static str = "[BUSY]"; // Đang bận xử lý dữ liệu lớn
    pub const PING: &'static str = "[PING]"; // Nhịp tim kiểm tra sức khỏe mạng
    pub const TIME: &'static str = "[TIME]"; // Bộ đếm thời gian / Timestamp
    pub const PROC: &'static str = "[PROC]"; // Tiến trình tính toán / CPU

    // Cấu hình & Hệ thống (System & Configuration)
    pub const CONF: &'static str = "[CONF]"; // Tệp cấu hình / Cài đặt
    pub const CORE: &'static str = "[CORE]"; // Luồng nhân lõi / Engine
    pub const TOOL: &'static str = "[TOOL]"; // Công cụ bảo trì / Tiện ích
    pub const DEVS: &'static str = "[DEVS]"; // Thiết bị phần cứng / Môi trường Dev
    pub const TEST: &'static str = "[TEST]"; // Kiểm thử tự động / Benchmark
    pub const INIT: &'static str = "[INIT]"; // Khởi tạo ban đầu
    pub const EXIT: &'static str = "[EXIT]"; // Thoát ứng dụng / Shutdown
    pub const KILL: &'static str = "[KILL]"; // Buộc dừng tiến trình

    // Tệp tin & Lưu trữ (File & Storage)
    pub const DIRS: &'static str = "[DIRS]"; // Thư mục tập tin
    pub const FILE: &'static str = "[FILE]"; // Tệp dữ liệu đơn
    pub const DOCS: &'static str = "[DOCS]"; // Tài liệu / Văn bản hướng dẫn
    pub const PACK: &'static str = "[PACK]"; // Gói cài đặt / Dependencies
    pub const ARCH: &'static str = "[ARCH]"; // Tệp nén / Kho lưu trữ
    pub const ROOT: &'static str = "[ROOT]"; // Thư mục gốc dự án

    // Media & Âm thanh (Media & Audio)
    pub const MEDA: &'static str = "[MEDA]"; // Tệp đa phương tiện chung
    pub const AUDI: &'static str = "[AUDI]"; // Luồng âm thanh / Nhạc
    pub const RECD: &'static str = "[RECD]"; // Thu âm / Micro mở
    pub const IMAG: &'static str = "[IMAG]"; // Hình ảnh / Đồ họa
    pub const SONG: &'static str = "[SONG]"; // Bài hát / Track nhạc
    pub const VOIC: &'static str = "[VOIC]"; // Giọng nói AI / Dubbing

    // Điều khiển & Tiến trình (Execution & Control)
    pub const EXEC: &'static str = "[EXEC]"; // Khởi chạy lệnh thực thi
    pub const PLAY: &'static str = "[PLAY]"; // Đang phát media
    pub const PAUS: &'static str = "[PAUS]"; // Tạm dừng phát
    pub const STOP: &'static str = "[STOP]"; // Dừng phát hẳn
    pub const NEXT: &'static str = "[NEXT]"; // Chuyển mục tiếp theo
    pub const PREV: &'static str = "[PREV]"; // Quay lại mục trước
    pub const STEP: &'static str = "[STEP]"; // Bước kế tiếp
    pub const SYNC: &'static str = "[SYNC]"; // Đồng bộ dữ liệu nền

    // Mạng & Bảo mật (Network & Security)
    pub const NETW: &'static str = "[NETW]"; // Kết nối mạng / Socket
    pub const APIS: &'static str = "[APIS]"; // Endpoint API / HTTP
    pub const LINK: &'static str = "[LINK]"; // Đường dẫn URL ngoài
    pub const USER: &'static str = "[USER]"; // Tài khoản người dùng
    pub const AUTH: &'static str = "[AUTH]"; // Xác thực quyền / Token
    pub const CERT: &'static str = "[CERT]"; // Chứng chỉ SSL / TLS
    pub const NODE: &'static str = "[NODE]"; // Node máy chủ / Cluster
    pub const HOST: &'static str = "[HOST]"; // Máy chủ đích
    pub const PORT: &'static str = "[PORT]"; // Cổng dịch vụ
    pub const PIPE: &'static str = "[PIPE]"; // Đường ống luồng IPC

    // =========================================================================
    // 2. CHUẨN SYMBOL-BASED 1 KÝ TỰ: [X] (TỔNG ĐỘ RỘNG CỐ ĐỊNH: 3 CELL)
    // =========================================================================

    // Điều hướng & Cây thư mục (Navigation & Hierarchy)
    pub const POINTER: &'static str = "[>]"; // Con trỏ đang chọn / Active
    pub const EXPAND: &'static str = "[+]";  // Thư mục đóng (bấm bung) / Thêm mới
    pub const COLLAPSE: &'static str = "[-]"; // Thư mục mở / Thu gọn
    pub const PATH: &'static str = "[/]";    // Thư mục con / Phân cấp
    pub const HOME: &'static str = "[~]";    // Thư mục Home / Vùng tạm

    // Trạng thái lựa chọn (Selection & Form Controls)
    pub const CHECKED: &'static str = "[x]"; // Checkbox: Đã chọn
    pub const UNCHECKED: &'static str = "[ ]"; // Checkbox: Chưa chọn
    pub const RADIO_ACTIVE: &'static str = "[*]"; // Radio: Đang chọn / Star
    pub const RADIO_INACTIVE: &'static str = "[.]"; // Radio: Chưa chọn

    // Thông báo & Cảnh báo (Prompts & Notices)
    pub const INFO_CHAR: &'static str = "[i]"; // Trợ giúp thông tin
    pub const ALERT_CHAR: &'static str = "[!]"; // Cảnh báo khẩn
    pub const HELP_CHAR: &'static str = "[?]"; // Câu hỏi xác nhận
    pub const ERROR_CHAR: &'static str = "[#]"; // Lỗi xung đột / Mã số

    // Tài nguyên & Tiến trình (Resources & Process)
    pub const USER_CHAR: &'static str = "[@]"; // Người dùng / Định danh
    pub const JOB_CHAR: &'static str = "[&]"; // Tác vụ chạy ngầm
    pub const PERCENT_CHAR: &'static str = "[%]"; // Tiến độ / Tải trọng
    pub const COMMAND_CHAR: &'static str = "[$]"; // Lệnh Shell / CLI

    // Media & Thao tác (Media & Navigation Chars)
    pub const PAUSE_CHAR: &'static str = "[=]"; // Tạm dừng media
    pub const UP_CHAR: &'static str = "[^]";   // Nhảy lên đầu trang
    pub const DOWN_CHAR: &'static str = "[v]"; // Cuộn xuống cuối trang
    pub const RECORD_CHAR: &'static str = "[o]"; // Ghi âm / Nguồn bật

    // =========================================================================
    // 3. TIỆN ÍCH ZERO-ALLOCATION RENDER
    // =========================================================================

    /// Ghép Tag với nội dung kèm đúng 1 dấu cách đệm chuẩn UI
    #[inline]
    pub fn format(tag: &'static str, message: &str) -> String {
        format!("{} {}", tag, message)
    }

    /// Tạo Span được bảo vệ Modifier::BOLD (giữ nguyên độ thẳng hàng)
    #[inline]
    pub fn span(tag: &'static str, style: Style) -> Span<'static> {
        let safe_style = style.remove_modifier(Modifier::BOLD);
        Span::styled(tag, safe_style)
    }

    /// Ghép Tag và Message thành Line với style phân tầng độc lập
    #[inline]
    pub fn line(
        tag: &'static str,
        tag_color: Color,
        message: impl Into<String>,
        msg_style: Style,
    ) -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("{} ", tag), Style::default().fg(tag_color)),
            Span::styled(message.into(), msg_style),
        ])
    }

    /// Trả về màu sắc Theme mặc định tương ứng với mức độ nghiêm trọng (Zero-cost matching)
    #[inline]
    pub fn color_for(tag: &str) -> Color {
        match tag {
            Self::INFO | Self::INFO_CHAR | Self::LINK | Self::APIS | Self::USER => crate::Theme::ACCENT,
            Self::WARN | Self::ALERT_CHAR | Self::TIME | "[LOCK]" => crate::Theme::WARNING,
            Self::FAIL | Self::ERROR_CHAR | Self::KILL => crate::Theme::ERROR,
            Self::PASS | Self::DONE | Self::CHECKED => crate::Theme::SUCCESS,
            Self::CONF | Self::CORE | Self::EXEC | Self::POINTER | Self::INIT => crate::Theme::PRIMARY,
            Self::WAIT | Self::PAUS | Self::PAUSE_CHAR => crate::Theme::ORANGE,
            Self::AUDI | Self::SONG | Self::VOIC | Self::SYNC => crate::Theme::CYAN,
            Self::DEVS | Self::NODE | Self::CERT | Self::PACK | Self::ARCH | Self::NETW => crate::Theme::PURPLE,
            Self::PROC | Self::PIPE => crate::Theme::INDIGO,
            "[SKIP]" => crate::Theme::MUTED,
            _ => crate::Theme::SECONDARY,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag4_exact_length_guard() {
        // Guardrail: Toàn bộ Tag 4 ký tự BẮT BUỘC phải có độ dài chính xác 6 ký tự
        let all_tag4 = [
            Tags::INFO, Tags::WARN, Tags::FAIL, Tags::PASS, Tags::DONE, Tags::WAIT, Tags::IDLE, Tags::BUSY,
            Tags::PING, Tags::TIME, Tags::PROC, Tags::CONF, Tags::CORE, Tags::TOOL, Tags::DEVS, Tags::TEST,
            Tags::INIT, Tags::EXIT, Tags::KILL, Tags::DIRS, Tags::FILE, Tags::DOCS, Tags::PACK, Tags::ARCH,
            Tags::ROOT, Tags::MEDA, Tags::AUDI, Tags::RECD, Tags::IMAG, Tags::SONG, Tags::VOIC, Tags::EXEC,
            Tags::PLAY, Tags::PAUS, Tags::STOP, Tags::NEXT, Tags::PREV, Tags::STEP, Tags::SYNC, Tags::NETW,
            Tags::APIS, Tags::LINK, Tags::USER, Tags::AUTH, Tags::CERT, Tags::NODE, Tags::HOST, Tags::PORT,
            Tags::PIPE,
        ];

        for tag in all_tag4 {
            assert_eq!(
                tag.chars().count(),
                6,
                "Tag '{}' vi phạm ranh giới độ dài: phải đúng 6 ký tự!",
                tag
            );
            assert!(tag.starts_with('['), "Tag '{}' phải bắt đầu bằng '['", tag);
            assert!(tag.ends_with(']'), "Tag '{}' phải kết thúc bằng ']'", tag);
        }
    }

    #[test]
    fn test_tag1_exact_length_guard() {
        // Guardrail: Toàn bộ Tag 1 ký tự BẮT BUỘC phải có độ dài chính xác 3 ký tự
        let all_tag1 = [
            Tags::POINTER, Tags::EXPAND, Tags::COLLAPSE, Tags::PATH, Tags::HOME,
            Tags::CHECKED, Tags::UNCHECKED, Tags::RADIO_ACTIVE, Tags::RADIO_INACTIVE,
            Tags::INFO_CHAR, Tags::ALERT_CHAR, Tags::HELP_CHAR, Tags::ERROR_CHAR,
            Tags::USER_CHAR, Tags::JOB_CHAR, Tags::PERCENT_CHAR, Tags::COMMAND_CHAR,
            Tags::PAUSE_CHAR, Tags::UP_CHAR, Tags::DOWN_CHAR, Tags::RECORD_CHAR,
        ];

        for tag in all_tag1 {
            assert_eq!(
                tag.chars().count(),
                3,
                "Tag '{}' vi phạm ranh giới độ dài: phải đúng 3 ký tự!",
                tag
            );
            assert!(tag.starts_with('['), "Tag '{}' phải bắt đầu bằng '['", tag);
            assert!(tag.ends_with(']'), "Tag '{}' phải kết thúc bằng ']'", tag);
        }
    }

    #[test]
    fn test_tags_format_and_span() {
        assert_eq!(Tags::format(Tags::INFO, "System ready"), "[INFO] System ready");
        let span = Tags::span(Tags::PASS, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD));
        assert!(!span.style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(span.content, "[PASS]");
    }
}
