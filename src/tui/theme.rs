use ratatui::style::Color;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeColor {
    Background,
    Foreground,
    Selection,
    SelectionForeground,
    Heading,
    Accent,
    Shortcut,
    LogoPrimary,
    LogoSecondary,
    Healthy,
    Warning,
    Error,
    Muted,
}

pub struct K9sDark;

impl K9sDark {
    pub const NAME: &'static str = "k9s-dark";

    pub fn color(color: ThemeColor) -> Color {
        match color {
            ThemeColor::Background => Color::Rgb(0, 0, 0),
            ThemeColor::Foreground => Color::Rgb(216, 222, 233),
            ThemeColor::Selection => Color::Rgb(129, 200, 248),
            ThemeColor::SelectionForeground => Color::Rgb(0, 0, 0),
            ThemeColor::Heading => Color::Rgb(0, 215, 215),
            ThemeColor::Accent => Color::Rgb(255, 215, 0),
            ThemeColor::Shortcut => Color::Rgb(0, 215, 255),
            ThemeColor::LogoPrimary => Color::Rgb(255, 175, 0),
            ThemeColor::LogoSecondary => Color::Rgb(255, 215, 0),
            ThemeColor::Healthy => Color::Rgb(95, 215, 135),
            ThemeColor::Warning => Color::Rgb(255, 175, 0),
            ThemeColor::Error => Color::Rgb(255, 95, 95),
            ThemeColor::Muted => Color::Rgb(128, 128, 128),
        }
    }
}
