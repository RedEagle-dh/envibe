use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::tui::styles::Theme;

pub struct HelpPopup<'a> {
    theme: &'a Theme,
}

impl<'a> HelpPopup<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self { theme }
    }
}

impl Widget for HelpPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear the area first
        Clear.render(area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.theme.border_style(true))
            .title(Span::styled(" Help ", self.theme.title_style()))
            .title_alignment(Alignment::Center);

        let key_style = Style::default()
            .fg(self.theme.title)
            .add_modifier(Modifier::BOLD);
        let desc_style = Style::default().fg(self.theme.fg);

        let help_text = vec![
            Line::from(vec![
                Span::styled("Global", Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)),
            ]),
            Line::from(vec![
                Span::styled("  q, Ctrl+C  ", key_style),
                Span::styled("Quit application", desc_style),
            ]),
            Line::from(vec![
                Span::styled("  Tab        ", key_style),
                Span::styled("Cycle focus between panels", desc_style),
            ]),
            Line::from(vec![
                Span::styled("  ?          ", key_style),
                Span::styled("Toggle this help", desc_style),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Navigation", Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)),
            ]),
            Line::from(vec![
                Span::styled("  j/k, ↑/↓   ", key_style),
                Span::styled("Navigate items", desc_style),
            ]),
            Line::from(vec![
                Span::styled("  Enter      ", key_style),
                Span::styled("Select project / Toggle service", desc_style),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Services", Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)),
            ]),
            Line::from(vec![
                Span::styled("  Space      ", key_style),
                Span::styled("Toggle service (start/stop)", desc_style),
            ]),
            Line::from(vec![
                Span::styled("  r          ", key_style),
                Span::styled("Restart service", desc_style),
            ]),
            Line::from(vec![
                Span::styled("  a          ", key_style),
                Span::styled("Start all services", desc_style),
            ]),
            Line::from(vec![
                Span::styled("  s          ", key_style),
                Span::styled("Stop all services", desc_style),
            ]),
            Line::from(vec![
                Span::styled("  l          ", key_style),
                Span::styled("Focus logs for service", desc_style),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Console", Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)),
            ]),
            Line::from(vec![
                Span::styled("  PgUp/PgDn  ", key_style),
                Span::styled("Scroll logs", desc_style),
            ]),
            Line::from(vec![
                Span::styled("  f          ", key_style),
                Span::styled("Toggle follow mode", desc_style),
            ]),
            Line::from(vec![
                Span::styled("  c          ", key_style),
                Span::styled("Clear logs", desc_style),
            ]),
        ];

        let paragraph = Paragraph::new(help_text).block(block);
        paragraph.render(area, buf);
    }
}
