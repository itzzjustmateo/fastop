use ratatui::style::Color;

/// Dimmed color for secondary text and panel borders.
pub(crate) const MUTED: Color = Color::DarkGray;
/// Accent color used for titles, highlights and the selected row.
pub(crate) const ACCENT: Color = Color::Cyan;
/// Color used for network upload (transmitted) figures.
pub(crate) const UPLOAD: Color = Color::LightMagenta;

/// Maps a percentage to a green/yellow/red traffic-light color.
pub(crate) fn usage_color(usage: f32) -> Color {
    match usage {
        u if u >= 90.0 => Color::Red,
        u if u >= 70.0 => Color::Yellow,
        _ => Color::Green,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_color_thresholds() {
        assert_eq!(usage_color(0.0), Color::Green);
        assert_eq!(usage_color(69.9), Color::Green);
        assert_eq!(usage_color(70.0), Color::Yellow);
        assert_eq!(usage_color(89.9), Color::Yellow);
        assert_eq!(usage_color(90.0), Color::Red);
    }
}
