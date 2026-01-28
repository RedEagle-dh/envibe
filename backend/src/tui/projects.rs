use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget, Widget},
};

use crate::config::Project;
use crate::tui::styles::Theme;

pub struct ProjectsPanel<'a> {
    projects: &'a [Project],
    theme: &'a Theme,
    focused: bool,
}

impl<'a> ProjectsPanel<'a> {
    pub fn new(projects: &'a [Project], theme: &'a Theme, focused: bool) -> Self {
        Self {
            projects,
            theme,
            focused,
        }
    }
}

impl StatefulWidget for ProjectsPanel<'_> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.theme.border_style(self.focused))
            .title(Span::styled(" Projects ", self.theme.title_style()));

        let items: Vec<ListItem> = self
            .projects
            .iter()
            .enumerate()
            .map(|(i, project)| {
                let selected = state.selected() == Some(i);

                let indicator = if project.config.is_some() {
                    "●"
                } else if project.has_docker_compose {
                    "○"
                } else {
                    " "
                };

                let style = if selected {
                    self.theme.highlight_style()
                } else {
                    self.theme.base_style()
                };

                let content = Line::from(vec![
                    Span::styled(
                        indicator,
                        Style::default().fg(if project.config.is_some() {
                            self.theme.status_running
                        } else {
                            self.theme.status_stopped
                        }),
                    ),
                    Span::raw(" "),
                    Span::styled(&project.name, style),
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

impl Widget for ProjectsPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = ListState::default();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}
