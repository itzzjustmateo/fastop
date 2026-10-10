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

/// Maps a temperature (in degrees Celsius) to a cool/warm/hot color.
pub(crate) fn temperature_color(celsius: f32) -> Color {
    if celsius >= 80.0 {
        Color::Red
    } else if celsius >= 60.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

/// Maps a battery charge percentage to a color (a low charge is a warning).
pub(crate) fn charge_color(percentage: f32) -> Color {
    if percentage <= 15.0 {
        Color::Red
    } else if percentage <= 40.0 {
        Color::Yellow
    } else {
        Color::Green
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

    #[test]
    fn temperature_color_thresholds() {
        assert_eq!(temperature_color(35.0), Color::Green);
        assert_eq!(temperature_color(59.9), Color::Green);
        assert_eq!(temperature_color(60.0), Color::Yellow);
        assert_eq!(temperature_color(79.9), Color::Yellow);
        assert_eq!(temperature_color(80.0), Color::Red);
    }

    #[test]
    fn charge_color_inverts_usage_semantics() {
        assert_eq!(charge_color(5.0), Color::Red);
        assert_eq!(charge_color(15.0), Color::Red);
        assert_eq!(charge_color(40.0), Color::Yellow);
        assert_eq!(charge_color(80.0), Color::Green);
    }
}
