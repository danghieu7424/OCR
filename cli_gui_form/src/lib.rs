//! # cli_gui_form
//!
//! A minimal, professional, zero-noise TUI Form & Design System library for Rust powered by [`ratatui`] and [`crossterm`].
//!
//! ## Key Features
//! - **Focus & Form Management**: Seamless Tab / Shift+Tab / Arrow navigation with automatic viewport scrolling.
//! - **Rich Widget Suite**: Text / Password Inputs with inline cursor, Checkboxes, Radios, Vertical Lists / Menus, Panels / Cards, Action Buttons, Shimmer progress bars, Tabs, Status Bars, and Task spinners.
//! - **Zero-Allocation Icon System**: 22 clean Unicode icons (`\u{FE0E}`) with pre-padded 2-cell spacing.
//! - **Overflow Protection**: Hard truncation guards preventing frame breaks when text length exceeds terminal width.
//! - **Dark Minimalist Aesthetic**: High-contrast, glassmorphic dark palette with balanced neutral grayscale.
//!
//! ## Quickstart
//!
//! ```no_run
//! use cli_gui_form::{FormManager, InputWidget, InputMode, CheckboxWidget, ListWidget, ButtonWidget};
//! use ratatui::style::Color;
//!
//! let mut form = FormManager::new();
//! form.add_widget(Box::new(InputWidget::new("username", "Username", InputMode::Text)));
//! form.add_widget(Box::new(InputWidget::new("password", "Password", InputMode::Password)));
//! form.add_widget(Box::new(CheckboxWidget::new("remember", "Remember Me", true)));
//! form.add_widget(Box::new(ListWidget::new("role").with_item("Admin").with_item("User")));
//! form.add_widget(Box::new(ButtonWidget::new("submit", "SUBMIT", Color::Blue, Color::White)));
//! ```

pub mod icons;
pub mod manager;
pub mod tags;
pub mod theme;
pub mod traits;
pub mod widgets;

pub use icons::Icons;
pub use manager::FormManager;
pub use tags::Tags;
pub use theme::Theme;
pub use traits::{EventResult, FormValue, FormWidget};
pub use widgets::{
    ButtonWidget, CardItem, CardWidget, CheckboxWidget, EditMode, EditableListWidget, InputMode, InputWidget,
    ListItem, ListWidget, RadioWidget, SelectOption, SelectWidget, ShimmerWidget, StatusBarWidget, TabsWidget,
    ProgressStyle, SpinnerType, TaskState, TaskWidget,
};