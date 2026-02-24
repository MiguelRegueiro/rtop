use ratatui::style::Color;

/// Default color mappings for the application
#[allow(dead_code)]
pub fn get_default_color(
    default_color: Color,
    dark_color: Color,
    light_color: Color,
    mono_color: Color,
) -> Color {
    match (default_color, dark_color, light_color, mono_color) {
        (Color::Cyan, _, _, _) => Color::Cyan,
        (_, Color::Blue, _, _) => Color::Blue,
        (_, _, Color::White, _) => Color::White,
        (Color::Green, _, _, _) => Color::Green,
        (Color::Yellow, _, _, _) => Color::Yellow,
        (Color::Red, _, _, _) => Color::Red,
        (Color::Magenta, _, _, _) => Color::Magenta,
        (Color::Blue, _, _, _) => Color::Blue,
        (Color::LightRed, _, _, _) => Color::LightRed,
        (Color::LightGreen, _, _, _) => Color::LightGreen,
        (Color::LightYellow, _, _, _) => Color::LightYellow,
        (Color::LightBlue, _, _, _) => Color::LightBlue,
        (Color::LightMagenta, _, _, _) => Color::LightMagenta,
        (Color::LightCyan, _, _, _) => Color::LightCyan,
        (Color::Gray, _, _, _) => Color::Gray,
        (Color::DarkGray, _, _, _) => Color::DarkGray,
        _ => Color::White,
    }
}
