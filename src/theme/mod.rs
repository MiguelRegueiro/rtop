use crate::data::snapshot::ColorScheme;
use ratatui::style::Color;

mod default;

#[derive(Debug, Clone)]
pub struct Theme {
    pub color_scheme: ColorScheme,
}

impl Theme {
    pub fn new(color_scheme: ColorScheme) -> Self {
        Self {
            color_scheme: Self::canonicalize_color_scheme(color_scheme),
        }
    }

    pub fn canonicalize_color_scheme(color_scheme: ColorScheme) -> ColorScheme {
        match color_scheme {
            // Legacy/low-contrast variants are normalized to readable modern themes.
            ColorScheme::Light => ColorScheme::Default,
            ColorScheme::Monochrome => ColorScheme::Dark,
            ColorScheme::SolarizedLight => ColorScheme::SolarizedDark,
            _ => color_scheme,
        }
    }

    pub fn cycle() -> &'static [ColorScheme] {
        &[
            ColorScheme::Default,
            ColorScheme::Dark,
            ColorScheme::Nord,
            ColorScheme::SolarizedDark,
            ColorScheme::Gruvbox,
            ColorScheme::Rtop,
        ]
    }

    pub fn text_style(&self) -> ratatui::style::Style {
        ratatui::style::Style::default().fg(self.get_color(Color::White))
    }

    pub fn get_color(&self, default_color: Color) -> Color {
        match self.color_scheme {
            ColorScheme::Default => self.get_graphite_color(default_color),
            ColorScheme::Dark => self.get_midnight_color(default_color),
            ColorScheme::Light => self.get_graphite_color(default_color),
            ColorScheme::Monochrome => self.get_midnight_color(default_color),
            ColorScheme::Nord => self.get_nord_color(default_color),
            ColorScheme::SolarizedDark => self.get_solarized_dark_color(default_color),
            ColorScheme::SolarizedLight => self.get_solarized_dark_color(default_color),
            ColorScheme::Gruvbox => self.get_gruvbox_color(default_color),
            ColorScheme::Rtop => self.get_rtop_color(default_color),
        }
    }

    fn get_graphite_color(&self, default_color: Color) -> Color {
        match default_color {
            Color::White => Color::Rgb(228, 236, 245),
            Color::Black => Color::Rgb(11, 17, 23),
            Color::DarkGray => Color::Rgb(29, 41, 58),
            Color::Gray => Color::Rgb(140, 158, 182),
            Color::Cyan => Color::Rgb(94, 213, 221),
            Color::Blue => Color::Rgb(126, 170, 255),
            Color::Green => Color::Rgb(108, 212, 149),
            Color::Yellow => Color::Rgb(243, 197, 109),
            Color::Red => Color::Rgb(241, 126, 126),
            Color::Magenta => Color::Rgb(198, 157, 255),
            Color::LightRed => Color::Rgb(255, 150, 150),
            Color::LightGreen => Color::Rgb(137, 230, 170),
            Color::LightYellow => Color::Rgb(255, 216, 133),
            Color::LightBlue => Color::Rgb(157, 194, 255),
            Color::LightMagenta => Color::Rgb(217, 182, 255),
            Color::LightCyan => Color::Rgb(124, 227, 234),
            _ => default_color,
        }
    }

    fn get_midnight_color(&self, default_color: Color) -> Color {
        match default_color {
            Color::White => Color::Rgb(230, 237, 247),
            Color::Black => Color::Rgb(9, 12, 20),
            Color::DarkGray => Color::Rgb(20, 28, 45),
            Color::Gray => Color::Rgb(124, 143, 171),
            Color::Cyan => Color::Rgb(93, 204, 226),
            Color::Blue => Color::Rgb(106, 158, 255),
            Color::Green => Color::Rgb(116, 215, 155),
            Color::Yellow => Color::Rgb(247, 204, 117),
            Color::Red => Color::Rgb(244, 130, 130),
            Color::Magenta => Color::Rgb(200, 151, 255),
            Color::LightRed => Color::Rgb(255, 154, 154),
            Color::LightGreen => Color::Rgb(143, 233, 175),
            Color::LightYellow => Color::Rgb(255, 224, 140),
            Color::LightBlue => Color::Rgb(159, 199, 255),
            Color::LightMagenta => Color::Rgb(222, 189, 255),
            Color::LightCyan => Color::Rgb(123, 222, 239),
            _ => default_color,
        }
    }

    fn get_nord_color(&self, default_color: Color) -> Color {
        match default_color {
            Color::White => Color::Rgb(229, 233, 240),
            Color::Black => Color::Rgb(46, 52, 64),
            Color::DarkGray => Color::Rgb(59, 66, 82),
            Color::Gray => Color::Rgb(129, 161, 193),
            Color::Cyan => Color::Rgb(136, 192, 208),
            Color::Blue => Color::Rgb(129, 161, 193),
            Color::Green => Color::Rgb(163, 190, 140),
            Color::Yellow => Color::Rgb(235, 203, 139),
            Color::Red => Color::Rgb(191, 97, 106),
            Color::Magenta => Color::Rgb(180, 142, 173),
            Color::LightRed => Color::Rgb(219, 129, 139),
            Color::LightGreen => Color::Rgb(186, 214, 164),
            Color::LightYellow => Color::Rgb(245, 219, 161),
            Color::LightBlue => Color::Rgb(159, 189, 217),
            Color::LightMagenta => Color::Rgb(200, 165, 195),
            Color::LightCyan => Color::Rgb(164, 208, 221),
            _ => default_color,
        }
    }

    fn get_solarized_dark_color(&self, default_color: Color) -> Color {
        match default_color {
            Color::White => Color::Rgb(238, 232, 213),
            Color::Black => Color::Rgb(0, 43, 54),
            Color::DarkGray => Color::Rgb(7, 54, 66),
            Color::Gray => Color::Rgb(88, 110, 117),
            Color::Cyan => Color::Rgb(42, 161, 152),
            Color::Blue => Color::Rgb(38, 139, 210),
            Color::Green => Color::Rgb(133, 153, 0),
            Color::Yellow => Color::Rgb(181, 137, 0),
            Color::Red => Color::Rgb(220, 50, 47),
            Color::Magenta => Color::Rgb(211, 54, 130),
            Color::LightRed => Color::Rgb(234, 98, 93),
            Color::LightGreen => Color::Rgb(152, 174, 15),
            Color::LightYellow => Color::Rgb(201, 159, 23),
            Color::LightBlue => Color::Rgb(73, 155, 218),
            Color::LightMagenta => Color::Rgb(220, 87, 148),
            Color::LightCyan => Color::Rgb(74, 182, 173),
            _ => default_color,
        }
    }

    fn get_gruvbox_color(&self, default_color: Color) -> Color {
        match default_color {
            Color::White => Color::Rgb(235, 219, 178),
            Color::Black => Color::Rgb(29, 32, 33),
            Color::DarkGray => Color::Rgb(60, 56, 54),
            Color::Gray => Color::Rgb(168, 153, 132),
            Color::Cyan => Color::Rgb(142, 192, 124),
            Color::Blue => Color::Rgb(131, 165, 152),
            Color::Green => Color::Rgb(184, 187, 38),
            Color::Yellow => Color::Rgb(250, 189, 47),
            Color::Red => Color::Rgb(251, 73, 52),
            Color::Magenta => Color::Rgb(211, 134, 155),
            Color::LightRed => Color::Rgb(255, 122, 102),
            Color::LightGreen => Color::Rgb(204, 207, 67),
            Color::LightYellow => Color::Rgb(255, 211, 90),
            Color::LightBlue => Color::Rgb(158, 193, 179),
            Color::LightMagenta => Color::Rgb(227, 160, 178),
            Color::LightCyan => Color::Rgb(165, 209, 146),
            _ => default_color,
        }
    }

    fn get_rtop_color(&self, default_color: Color) -> Color {
        match default_color {
            Color::White => Color::Rgb(235, 242, 255),
            Color::Black => Color::Rgb(10, 14, 24),
            Color::DarkGray => Color::Rgb(29, 37, 58),
            Color::Gray => Color::Rgb(122, 141, 173),
            Color::Cyan => Color::Rgb(74, 235, 220),
            Color::Blue => Color::Rgb(94, 164, 255),
            Color::Green => Color::Rgb(122, 241, 156),
            Color::Yellow => Color::Rgb(255, 212, 92),
            Color::Red => Color::Rgb(255, 124, 124),
            Color::Magenta => Color::Rgb(214, 149, 255),
            Color::LightRed => Color::Rgb(255, 155, 155),
            Color::LightGreen => Color::Rgb(154, 255, 184),
            Color::LightYellow => Color::Rgb(255, 228, 134),
            Color::LightBlue => Color::Rgb(131, 187, 255),
            Color::LightMagenta => Color::Rgb(229, 177, 255),
            Color::LightCyan => Color::Rgb(112, 247, 234),
            _ => default_color,
        }
    }
}
