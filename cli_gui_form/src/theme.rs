// --- PHÂN ĐOẠN: BẢNG MÀU VÀ GIAO DIỆN CHUẨN DESIGN.MD (MINIMAL TUI DESIGN SYSTEM) ---

use ratatui::style::Color;

/****
 * Module: Theme
 * Chức năng: Cung cấp bảng màu chuẩn Semantic Roles và Neutral Scale từ DESIGN.md.
 * Ranh giới bảo vệ: Hằng số biên dịch không phân bổ bộ nhớ động (Zero-Cost Abstraction).
 *            Áp dụng chuẩn phong cách Vercel/Linear: nền tối trầm, chữ xám/trắng, điểm nhấn Accent xanh dương.
 ****/
pub struct Theme;

impl Theme {
    // ----------------------------------------------------
    // 1. SEMANTIC ROLES (Màu theo ngữ nghĩa)
    // ----------------------------------------------------
    
    // Background: #0a0a0a (ANSI 232)
    pub const BG: Color = Color::Rgb(0x0a, 0x0a, 0x0a);

    // Foreground: #ededed (ANSI 255)
    pub const FG: Color = Color::Rgb(0xed, 0xed, 0xed);

    // Primary: #ffffff (ANSI 15)
    pub const PRIMARY: Color = Color::Rgb(0xff, 0xff, 0xff);

    // Secondary: #888888 (ANSI 245)
    pub const SECONDARY: Color = Color::Rgb(0x88, 0x88, 0x88);

    // Accent: #0070f3 (ANSI 33) - Vercel blue
    pub const ACCENT: Color = Color::Rgb(0x00, 0x70, 0xf3);

    // Success: #00c853 (ANSI 41)
    pub const SUCCESS: Color = Color::Rgb(0x00, 0xc8, 0x53);

    // Warning: #f5a623 (ANSI 214)
    pub const WARNING: Color = Color::Rgb(0xf5, 0xa6, 0x23);

    // Error: #ee0000 (ANSI 196)
    pub const ERROR: Color = Color::Rgb(0xee, 0x00, 0x00);

    // Muted: #555555 (ANSI 240)
    pub const MUTED: Color = Color::Rgb(0x55, 0x55, 0x55);

    // Surface: #1a1a1a (ANSI 234)
    pub const SURFACE: Color = Color::Rgb(0x1a, 0x1a, 0x1a);

    // ----------------------------------------------------
    // 2. EXTENDED ACCENT & SYSTEM PALETTE (Tránh lặp màu đơn điệu)
    // ----------------------------------------------------

    // Cyan / Teal: #50e3c2 (Vercel Cyan - Network, Latency, API Endpoints)
    pub const CYAN: Color = Color::Rgb(0x50, 0xe3, 0xc2);

    // Purple / Violet: #7928ca (Linear Violet - AI Inference, GraphQL, Plugins)
    pub const PURPLE: Color = Color::Rgb(0x79, 0x28, 0xca);

    // Magenta / Pink: #f81ce5 (Electric Pink - Auth, Secrets, Webhooks)
    pub const MAGENTA: Color = Color::Rgb(0xf8, 0x1c, 0xe5);

    // Orange / Amber: #ff8800 (Warm Amber - Pipelines, Queues, Workers)
    pub const ORANGE: Color = Color::Rgb(0xff, 0x88, 0x00);

    // Indigo: #5e6ad2 (Linear Indigo - Branches, Commits, Tasks)
    pub const INDIGO: Color = Color::Rgb(0x5e, 0x6a, 0xd2);

    // Emerald: #10b981 (Soft Green - Healthy Uptime, In-Memory DBs)
    pub const EMERALD: Color = Color::Rgb(0x10, 0xb9, 0x81);

    // Sky Blue: #38bdf8 (Cloud Infrastructure, Docker, K8s)
    pub const SKY: Color = Color::Rgb(0x38, 0xbd, 0xf8);

    // Critical: #ff0055 (Crimson Red - Fatal, Panic, Immediate Alert)
    pub const CRITICAL: Color = Color::Rgb(0xff, 0x00, 0x55);

    // Surface Elevated: #222222 (Card Header, Highlighted Row Background)
    pub const SURFACE_ELEVATED: Color = Color::Rgb(0x22, 0x22, 0x22);

    // Border Focus: #0070f3 (Viền khi ô form được kích hoạt)
    pub const BORDER_FOCUS: Color = Color::Rgb(0x00, 0x70, 0xf3);

    // ----------------------------------------------------
    // 3. NEUTRAL SCALE (Bảng thang độ xám)
    // ----------------------------------------------------
    pub const NEUTRAL_50: Color = Color::Rgb(0x1a, 0x1a, 0x1a);
    pub const NEUTRAL_100: Color = Color::Rgb(0x2a, 0x2a, 0x2a); // Borders, dividers
    pub const NEUTRAL_200: Color = Color::Rgb(0x44, 0x44, 0x44); // Disabled text (#444444)
    pub const NEUTRAL_300: Color = Color::Rgb(0x66, 0x66, 0x66); // Placeholder text (#666666)
    pub const NEUTRAL_400: Color = Color::Rgb(0x88, 0x88, 0x88); // Secondary text (#888888)
    pub const NEUTRAL_500: Color = Color::Rgb(0xed, 0xed, 0xed); // Body text

    // ----------------------------------------------------
    // 4. 16-STEP MONOCHROME / GRAYSCALE RAMP (#000000 -> #ffffff)
    // Tối ưu hoá: Kế thừa trực tiếp các mã màu đã có trong hệ thống,
    // bổ sung đầy đủ các nấc chuyển sắc để phục vụ gradient, shimmer, shadow.
    // ----------------------------------------------------
    pub const BLACK: Color = Color::Rgb(0x00, 0x00, 0x00); // #000000
    pub const GRAY_00: Color = Self::BLACK;
    pub const GRAY_000000: Color = Self::BLACK;

    pub const GRAY_11: Color = Color::Rgb(0x11, 0x11, 0x11); // #111111
    pub const GRAY_111111: Color = Self::GRAY_11;

    // #222222: Đã có sẵn SURFACE_ELEVATED
    pub const GRAY_22: Color = Self::SURFACE_ELEVATED; // #222222
    pub const GRAY_222222: Color = Self::SURFACE_ELEVATED;

    pub const GRAY_33: Color = Color::Rgb(0x33, 0x33, 0x33); // #333333
    pub const GRAY_333333: Color = Self::GRAY_33;

    // #444444: Đã có sẵn NEUTRAL_200
    pub const GRAY_44: Color = Self::NEUTRAL_200; // #444444
    pub const GRAY_444444: Color = Self::NEUTRAL_200;

    // #555555: Đã có sẵn MUTED
    pub const GRAY_55: Color = Self::MUTED; // #555555
    pub const GRAY_555555: Color = Self::MUTED;

    // #666666: Đã có sẵn NEUTRAL_300
    pub const GRAY_66: Color = Self::NEUTRAL_300; // #666666
    pub const GRAY_666666: Color = Self::NEUTRAL_300;

    pub const GRAY_77: Color = Color::Rgb(0x77, 0x77, 0x77); // #777777
    pub const GRAY_777777: Color = Self::GRAY_77;

    // #888888: Đã có sẵn SECONDARY / NEUTRAL_400
    pub const GRAY_88: Color = Self::SECONDARY; // #888888
    pub const GRAY_888888: Color = Self::SECONDARY;

    pub const GRAY_99: Color = Color::Rgb(0x99, 0x99, 0x99); // #999999
    pub const GRAY_999999: Color = Self::GRAY_99;

    pub const GRAY_AA: Color = Color::Rgb(0xaa, 0xaa, 0xaa); // #aaaaaa
    pub const GRAY_AAAAAA: Color = Self::GRAY_AA;

    pub const GRAY_BB: Color = Color::Rgb(0xbb, 0xbb, 0xbb); // #bbbbbb
    pub const GRAY_BBBBBB: Color = Self::GRAY_BB;

    pub const GRAY_CC: Color = Color::Rgb(0xcc, 0xcc, 0xcc); // #cccccc
    pub const GRAY_CCCCCC: Color = Self::GRAY_CC;

    pub const GRAY_DD: Color = Color::Rgb(0xdd, 0xdd, 0xdd); // #dddddd
    pub const GRAY_DDDDDD: Color = Self::GRAY_DD;

    pub const GRAY_EE: Color = Color::Rgb(0xee, 0xee, 0xee); // #eeeeee
    pub const GRAY_EEEEEE: Color = Self::GRAY_EE;

    pub const WHITE: Color = Color::Rgb(0xff, 0xff, 0xff); // #ffffff
    // #ffffff: Đã có sẵn PRIMARY
    pub const GRAY_FF: Color = Self::PRIMARY; // #ffffff
    pub const GRAY_FFFFFF: Color = Self::PRIMARY;

    /// Mảng hằng số 16 nấc màu xám liên tục từ tối nhất (#000000) đến sáng nhất (#ffffff)
    pub const GRAYSCALE_16: [Color; 16] = [
        Self::GRAY_00,
        Self::GRAY_11,
        Self::GRAY_22,
        Self::GRAY_33,
        Self::GRAY_44,
        Self::GRAY_55,
        Self::GRAY_66,
        Self::GRAY_77,
        Self::GRAY_88,
        Self::GRAY_99,
        Self::GRAY_AA,
        Self::GRAY_BB,
        Self::GRAY_CC,
        Self::GRAY_DD,
        Self::GRAY_EE,
        Self::GRAY_FF,
    ];

    // ----------------------------------------------------
    // 5. BUTTON COLOR PRESETS (Bảng phối màu an toàn chống co glyph)
    // Mỗi tuple gồm: (blur_bg, blur_fg, focused_bg, focused_fg)
    // ----------------------------------------------------
    /// Preset: Invert High-Contrast (Linear / Apple Dark Mode: Nền đen chữ xám -> Hover nền trắng chữ đen)
    pub const BTN_INVERT: (Color, Color, Color, Color) = (Self::BG, Self::SECONDARY, Self::PRIMARY, Self::BG);
    /// Preset: Solid Vercel Blue (Nền xanh chữ trắng -> Hover nền trắng chữ xanh)
    pub const BTN_ACCENT: (Color, Color, Color, Color) = (Self::ACCENT, Self::WHITE, Self::PRIMARY, Self::ACCENT);
    /// Preset: AI Violet Neural (Nền tím chữ trắng -> Hover nền trắng chữ tím)
    pub const BTN_PURPLE: (Color, Color, Color, Color) = (Self::PURPLE, Self::WHITE, Self::PRIMARY, Self::PURPLE);
    /// Preset: Emerald Success (Nền đen chữ xanh ngọc -> Hover nền xanh ngọc chữ đen)
    pub const BTN_EMERALD: (Color, Color, Color, Color) = (Self::BG, Self::EMERALD, Self::EMERALD, Self::BLACK);
    /// Preset: Minimal Ghost (GitHub CLI: Nền đen chữ xám -> Hover nền đen chữ xanh Accent)
    pub const BTN_GHOST: (Color, Color, Color, Color) = (Self::BG, Self::SECONDARY, Self::BG, Self::ACCENT);
    /// Preset: Amber Warning (Nền đen chữ cam -> Hover nền cam chữ đen)
    pub const BTN_AMBER: (Color, Color, Color, Color) = (Self::BG, Self::ORANGE, Self::ORANGE, Self::BLACK);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_colors() {
        assert_eq!(Theme::BG, Color::Rgb(10, 10, 10));
        assert_eq!(Theme::ACCENT, Color::Rgb(0, 112, 243));
        assert_eq!(Theme::SUCCESS, Color::Rgb(0, 200, 83));
        assert_eq!(Theme::CYAN, Color::Rgb(0x50, 0xe3, 0xc2));
        assert_eq!(Theme::PURPLE, Color::Rgb(0x79, 0x28, 0xca));
        assert_eq!(Theme::ORANGE, Color::Rgb(0xff, 0x88, 0x00));
        assert_eq!(Theme::INDIGO, Color::Rgb(0x5e, 0x6a, 0xd2));
    }

    #[test]
    fn test_grayscale_16_ramp() {
        assert_eq!(Theme::GRAYSCALE_16.len(), 16);
        for (i, &color) in Theme::GRAYSCALE_16.iter().enumerate() {
            let val = (i as u8) * 0x11;
            assert_eq!(color, Color::Rgb(val, val, val));
        }
        assert_eq!(Theme::BLACK, Color::Rgb(0x00, 0x00, 0x00));
        assert_eq!(Theme::WHITE, Color::Rgb(0xff, 0xff, 0xff));
        assert_eq!(Theme::GRAY_55, Theme::MUTED);
        assert_eq!(Theme::GRAY_88, Theme::SECONDARY);
        assert_eq!(Theme::GRAY_FF, Theme::PRIMARY);
    }
}
