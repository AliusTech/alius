pub mod app;
pub mod components;
pub mod config_panel;
pub mod init_wizard;
pub mod state;
pub mod theme;
pub mod workspace;

/// TUI test utilities (gated behind `testing` feature or `#[cfg(test)]`).
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use app::TuiApp;
pub use config_panel::run_config_panel;
pub use init_wizard::run_init_wizard;
