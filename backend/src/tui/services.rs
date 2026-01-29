use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget, Widget},
};

use crate::config::{Project, ServiceConfig, ServiceStatus};
use crate::tui::styles::Theme;
use crate::tui::ui::ServiceData;

pub struct ServicesPanel<'a> {
    project: Option<&'a Project>,
    service_data: &'a [ServiceData],
    theme: &'a Theme,
    focused: bool,
}

impl<'a> ServicesPanel<'a> {
    pub fn new(
        project: Option<&'a Project>,
        service_data: &'a [ServiceData],
        theme: &'a Theme,
        focused: bool,
    ) -> Self {
        Self {
            project,
            service_data,
            theme,
            focused,
        }
    }
}

impl StatefulWidget for ServicesPanel<'_> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let title = match self.project {
            Some(p) => format!(" {} - Services ", p.name),
            None => " Services ".to_string(),
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.theme.border_style(self.focused))
            .title(Span::styled(title, self.theme.title_style()));

        if self.service_data.is_empty() {
            let message = if self.project.is_some() {
                "No services configured"
            } else {
                "Select a project"
            };

            let inner = block.inner(area);
            block.render(area, buf);

            let line = Line::from(Span::styled(
                message,
                Style::default().fg(self.theme.border),
            ));
            buf.set_line(inner.x + 1, inner.y + 1, &line, inner.width - 2);
            return;
        }

        let items: Vec<ListItem> = self
            .service_data
            .iter()
            .enumerate()
            .map(|(i, data)| {
                let selected = state.selected() == Some(i);

                let (status, port) = match &data.state {
                    Some(s) => (s.status, s.port),
                    None => (ServiceStatus::Stopped, None),
                };

                let status_icon = match status {
                    ServiceStatus::Running => "▶",
                    ServiceStatus::Starting => "◐",
                    ServiceStatus::Stopping => "◑",
                    ServiceStatus::Stopped => "■",
                    ServiceStatus::Error => "✗",
                };

                let type_indicator = match &data.config {
                    ServiceConfig::Docker(_) => "🐳",
                    ServiceConfig::Process(_) => "⚡",
                    ServiceConfig::Compose(_) => "🐋",
                };

                let port_str = port.map(|p| format!(" ({})", p)).unwrap_or_default();

                let style = if selected {
                    self.theme.highlight_style()
                } else {
                    self.theme.base_style()
                };

                let content = Line::from(vec![
                    Span::styled(status_icon, self.theme.status_style(&status)),
                    Span::raw(" "),
                    Span::raw(type_indicator),
                    Span::raw(" "),
                    Span::styled(data.name.as_str(), style),
                    Span::styled(port_str, Style::default().fg(self.theme.log_timestamp)),
                ]);

                ListItem::new(content)
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .bg(self.theme.highlight_bg)
                    .add_modifier(Modifier::BOLD),
            );

        StatefulWidget::render(list, area, buf, state);
    }
}

impl Widget for ServicesPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = ListState::default();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}
