use ratatui::style::{Color, Modifier, Style};

pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub highlight_bg: Color,
    pub highlight_fg: Color,
    pub border: Color,
    pub border_focused: Color,
    pub title: Color,
    pub status_running: Color,
    pub status_stopped: Color,
    pub status_starting: Color,
    pub status_error: Color,
    pub log_info: Color,
    pub log_error: Color,
    pub log_timestamp: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Reset,
            fg: Color::White,
            highlight_bg: Color::Rgb(50, 50, 80),
            highlight_fg: Color::White,
            border: Color::DarkGray,
            border_focused: Color::Cyan,
            title: Color::Cyan,
            status_running: Color::Green,
            status_stopped: Color::DarkGray,
            status_starting: Color::Yellow,
            status_error: Color::Red,
            log_info: Color::White,
            log_error: Color::Red,
            log_timestamp: Color::DarkGray,
        }
    }
}

impl Theme {
    pub fn base_style(&self) -> Style {
        Style::default().fg(self.fg).bg(self.bg)
    }

    pub fn highlight_style(&self) -> Style {
        Style::default()
            .fg(self.highlight_fg)
            .bg(self.highlight_bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn border_style(&self, focused: bool) -> Style {
        if focused {
            Style::default().fg(self.border_focused)
        } else {
            Style::default().fg(self.border)
        }
    }

    pub fn title_style(&self) -> Style {
        Style::default()
            .fg(self.title)
            .add_modifier(Modifier::BOLD)
    }

    pub fn status_style(&self, status: &crate::config::ServiceStatus) -> Style {
        use crate::config::ServiceStatus;
        let color = match status {
            ServiceStatus::Running => self.status_running,
            ServiceStatus::Stopped => self.status_stopped,
            ServiceStatus::Starting | ServiceStatus::Stopping => self.status_starting,
            ServiceStatus::Error => self.status_error,
        };
        Style::default().fg(color)
    }
}
