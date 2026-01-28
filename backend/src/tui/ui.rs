use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};

use crate::app::App;
use crate::config::{Project, ServiceConfig, ServiceState};
use crate::tui::console::ConsolePanel;
use crate::tui::help::HelpPopup;
use crate::tui::projects::ProjectsPanel;
use crate::tui::services::ServicesPanel;
use crate::tui::styles::Theme;

/// Owned service data for rendering
pub struct ServiceData {
    pub name: String,
    pub config: ServiceConfig,
    pub state: Option<ServiceState>,
}

pub fn render(frame: &mut Frame, app: &mut App) {
    let theme = Theme::default();

    // Create main layout
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),  // Projects panel
            Constraint::Percentage(30),  // Services panel
            Constraint::Percentage(50),  // Console panel
        ])
        .split(frame.area());

    // Render projects panel
    let projects_panel = ProjectsPanel::new(
        &app.projects,
        &theme,
        app.focused_panel == super::Panel::Projects,
    );
    frame.render_stateful_widget(
        projects_panel,
        main_chunks[0],
        &mut app.projects_state,
    );

    // Get current project's services (owned data to avoid borrow issues)
    let (current_project, service_data) = get_service_data(app);

    // Render services panel
    let services_panel = ServicesPanel::new(
        current_project.as_ref(),
        &service_data,
        &theme,
        app.focused_panel == super::Panel::Services,
    );
    frame.render_stateful_widget(
        services_panel,
        main_chunks[1],
        &mut app.services_state,
    );

    // Render console panel
    let console_panel = ConsolePanel::new(
        &app.logs,
        &theme,
        app.focused_panel == super::Panel::Console,
        app.follow_logs,
        app.log_scroll,
    );
    frame.render_widget(console_panel, main_chunks[2]);

    // Render help popup if active
    if app.show_help {
        let popup_area = centered_rect(60, 80, frame.area());
        let help = HelpPopup::new(&theme);
        frame.render_widget(help, popup_area);
    }
}

fn get_service_data(app: &App) -> (Option<Project>, Vec<ServiceData>) {
    let project = match app.current_project() {
        Some(p) => p.clone(),
        None => return (None, vec![]),
    };

    let config = match &project.config {
        Some(c) => c,
        None => return (Some(project), vec![]),
    };

    let project_services = app.state.get_project_services(&project.name);

    let data: Vec<ServiceData> = config
        .services
        .iter()
        .map(|(name, service_config)| {
            let state = project_services.and_then(|s| s.get(name)).cloned();
            ServiceData {
                name: name.clone(),
                config: service_config.clone(),
                state,
            }
        })
        .collect();

    (Some(project), data)
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
