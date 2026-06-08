use anyhow::Result;
use crossterm::{
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::Block;
use ratatui::Terminal;
use std::io::Stdout;

use crate::tui::theme;

pub struct TuiApp {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TuiApp {
    pub fn enter() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste
        )?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;
        Ok(Self { terminal })
    }

    pub fn restore(mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            DisableBracketedPaste
        )?;
        self.terminal.show_cursor()?;
        Ok(())
    }

    /// Temporarily restore the terminal to normal state, run a closure
    /// (typically a sub-TUI like init wizard or config panel), then re-enter
    /// the alternate screen with raw mode for the workspace TUI.
    pub fn suspend_for<F, R>(&mut self, f: F) -> Result<R>
    where
        F: FnOnce() -> Result<R>,
    {
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            DisableBracketedPaste
        )?;
        self.terminal.show_cursor()?;

        let result = f();

        enable_raw_mode()?;
        execute!(
            std::io::stdout(),
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste
        )?;
        let backend = CrosstermBackend::new(std::io::stdout());
        self.terminal = Terminal::new(backend)?;
        self.terminal.clear()?;

        result
    }

    pub fn draw(&mut self, f: impl FnOnce(&mut ratatui::Frame)) -> Result<()> {
        self.terminal.draw(|frame| {
            let area = frame.area();
            frame.render_widget(Block::default().style(theme::base()), area);
            f(frame);
        })?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }
}
