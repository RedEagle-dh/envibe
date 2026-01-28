pub mod console;
pub mod help;
pub mod projects;
pub mod services;
pub mod styles;
pub mod ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Projects,
    Services,
    Console,
}

impl Panel {
    pub fn next(self) -> Self {
        match self {
            Panel::Projects => Panel::Services,
            Panel::Services => Panel::Console,
            Panel::Console => Panel::Projects,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Panel::Projects => Panel::Console,
            Panel::Services => Panel::Projects,
            Panel::Console => Panel::Services,
        }
    }
}
