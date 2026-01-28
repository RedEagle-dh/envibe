use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::Span,
    widgets::{Block, Borders, Widget},
};

use crate::app::{LogEntry, LogKind};
use crate::tui::styles::Theme;

pub struct ConsolePanel<'a> {
    logs: &'a [LogEntry],
    theme: &'a Theme,
    focused: bool,
    follow: bool,
    scroll_offset: usize,
}

impl<'a> ConsolePanel<'a> {
    pub fn new(
        logs: &'a [LogEntry],
        theme: &'a Theme,
        focused: bool,
        follow: bool,
        scroll_offset: usize,
    ) -> Self {
        Self {
            logs,
            theme,
            focused,
            follow,
            scroll_offset,
        }
    }
}

impl Widget for ConsolePanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let follow_indicator = if self.follow { " [Follow] " } else { "" };
        let title = format!(" Console{} ", follow_indicator);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.theme.border_style(self.focused))
            .title(Span::styled(title, self.theme.title_style()));

        let inner = block.inner(area);
        block.render(area, buf);

        if self.logs.is_empty() {
            buf.set_stringn(
                inner.x + 1,
                inner.y + 1,
                "No logs yet...",
                inner.width.saturating_sub(2) as usize,
                Style::default().fg(self.theme.border),
            );
            return;
        }

        // Calculate visible lines
        let visible_height = inner.height as usize;
        if visible_height == 0 || inner.width == 0 {
            return;
        }
        let total_lines = self.logs.len();

        let start = if self.follow {
            total_lines.saturating_sub(visible_height)
        } else {
            self.scroll_offset.min(total_lines.saturating_sub(visible_height))
        };

        let error_style = Style::default().fg(self.theme.log_error);
        let info_style = Style::default().fg(self.theme.log_info);
        let normal_style = Style::default().fg(self.theme.fg);

        for (idx, log) in self
            .logs
            .iter()
            .skip(start)
            .take(visible_height)
            .enumerate()
        {
            let style = match log.kind {
                LogKind::Error => error_style,
                LogKind::Info => info_style,
                LogKind::Normal => normal_style,
            };

            let y = inner.y + idx as u16;
            buf.set_stringn(
                inner.x,
                y,
                log.text.as_str(),
                inner.width as usize,
                style,
            );
        }

        // Render scroll indicator if not following and there are more logs
        if !self.follow && total_lines > visible_height {
            let percentage = if total_lines <= visible_height {
                100
            } else {
                ((start + visible_height) * 100) / total_lines
            };
            let indicator = format!(" {}% ", percentage);
            let x = inner.x + inner.width.saturating_sub(indicator.len() as u16) - 1;
            buf.set_string(
                x,
                area.y,
                &indicator,
                Style::default().fg(self.theme.log_timestamp),
            );
        }
    }
}
