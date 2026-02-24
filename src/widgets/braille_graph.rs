use ratatui::widgets::canvas::{Canvas, Context, Points};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Widget},
};

/// A high-resolution graph widget using Braille patterns for sub-pixel precision
pub struct BrailleGraph<'a> {
    /// Data points to display
    data: &'a [u64],
    /// Optional block to wrap the widget in
    block: Option<Block<'a>>,
    /// Widget style
    style: Style,
    /// Value range (min, max)
    value_range: (f64, f64),
    /// Whether to fill the graph from bottom
    fill: bool,
    /// Whether to use gradient coloring based on value
    use_gradient: bool,
    /// Optional moving-average smoothing radius
    smoothing: usize,
    /// Draw a subtle baseline when not filling
    show_baseline: bool,
}

impl<'a> BrailleGraph<'a> {
    pub fn new(data: &'a [u64]) -> Self {
        Self {
            data,
            block: None,
            style: Style::default(),
            value_range: (0.0, 100.0),
            fill: false,
            use_gradient: false,
            smoothing: 0,
            show_baseline: true,
        }
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn value_range(mut self, min: f64, max: f64) -> Self {
        self.value_range = (min, max);
        self
    }

    pub fn fill(mut self, fill: bool) -> Self {
        self.fill = fill;
        self
    }

    pub fn use_gradient(mut self, use_gradient: bool) -> Self {
        self.use_gradient = use_gradient;
        self
    }

    pub fn smoothing(mut self, smoothing: usize) -> Self {
        self.smoothing = smoothing.min(8);
        self
    }

    pub fn show_baseline(mut self, show_baseline: bool) -> Self {
        self.show_baseline = show_baseline;
        self
    }

    fn smoothed_values(&self) -> Vec<f64> {
        if self.data.is_empty() {
            return Vec::new();
        }
        if self.smoothing == 0 {
            return self.data.iter().map(|v| *v as f64).collect();
        }

        let mut out = Vec::with_capacity(self.data.len());
        for idx in 0..self.data.len() {
            let start = idx.saturating_sub(self.smoothing);
            let end = (idx + self.smoothing + 1).min(self.data.len());
            let window = &self.data[start..end];
            let sum: u64 = window.iter().copied().sum();
            let avg = sum as f64 / window.len() as f64;
            out.push(avg);
        }
        out
    }
}

impl<'a> Widget for BrailleGraph<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Apply block if present
        let inner_area = match &self.block {
            Some(b) => {
                b.clone().render(area, buf);
                b.inner(area)
            }
            None => area,
        };

        if inner_area.width < 2 || inner_area.height < 2 {
            return;
        }

        // Create canvas for drawing
        let canvas = Canvas::default()
            .marker(ratatui::symbols::Marker::Braille)
            .background_color(self.style.bg.unwrap_or_default())
            .x_bounds([0.0, (inner_area.width - 1) as f64]) // Set x bounds to match display area
            .y_bounds([0.0, (inner_area.height - 1) as f64]) // Set y bounds to match display area
            .paint(|ctx| {
                if self.data.is_empty() {
                    return;
                }

                let (min_val, max_val) = self.value_range;
                let range = max_val - min_val;

                if range == 0.0 {
                    return;
                }

                let values = self.smoothed_values();
                let point_count = values.len();
                if point_count == 0 {
                    return;
                }
                let bottom_y = inner_area.height.saturating_sub(1) as f64;
                let base_color = self.style.fg.unwrap_or(Color::White);

                if self.show_baseline && !self.fill {
                    let baseline_color = dim_color(base_color, 0.28);
                    for x in 0..inner_area.width {
                        draw_point(ctx, x as f64, bottom_y, baseline_color);
                    }
                }

                let mut prev: Option<(f64, f64)> = None;
                for (i, value) in values.iter().enumerate() {
                    let x = if point_count == 1 {
                        0.0
                    } else {
                        i as f64 * (inner_area.width.saturating_sub(1) as f64)
                            / (point_count.saturating_sub(1) as f64)
                    };
                    // Normalize value to 0-1 range
                    let normalized_value = ((*value - min_val) / range).clamp(0.0, 1.0);

                    // Calculate y position (inverted because canvas y increases downward)
                    let y = bottom_y * (1.0 - normalized_value);

                    // Determine color based on value if using gradient
                    let color = if self.use_gradient {
                        calculate_gradient_color(normalized_value)
                    } else {
                        base_color
                    };

                    if let Some((prev_x, prev_y)) = prev {
                        draw_line(ctx, prev_x, prev_y, x, y, color);
                    } else {
                        draw_point(ctx, x, y, color);
                    }

                    if self.fill {
                        // Draw filled bar from bottom up to value
                        let mut y_cell = y.ceil() as u16;
                        let max_cell = inner_area.height.saturating_sub(1);
                        while y_cell <= max_cell {
                            draw_point(ctx, x, y_cell as f64, dim_color(color, 0.85));
                            y_cell = y_cell.saturating_add(1);
                        }
                    }

                    prev = Some((x, y));
                }
            });

        canvas.render(inner_area, buf);
    }
}

/// Helper function to calculate gradient color based on value
fn calculate_gradient_color(value: f64) -> Color {
    // Match bar gradient palette for a cohesive UI.
    if value < 0.5 {
        // #00ff87 -> #f9ff00
        let ratio = (value * 2.0) as f32;
        let r = (0.0 + (249.0 - 0.0) * ratio) as u8;
        let g = 255;
        let b = (135.0 + (0.0 - 135.0) * ratio) as u8;
        Color::Rgb(r, g, b)
    } else {
        // #f9ff00 -> #ff003c
        let ratio = ((value - 0.5) * 2.0) as f32;
        let r = (249.0 + (255.0 - 249.0) * ratio) as u8;
        let g = (255.0 + (0.0 - 255.0) * ratio) as u8;
        let b = (0.0 + (60.0 - 0.0) * ratio) as u8;
        Color::Rgb(r, g, b)
    }
}

fn draw_point(ctx: &mut Context<'_>, x: f64, y: f64, color: Color) {
    ctx.draw(&Points {
        coords: &[(x, y)],
        color,
    });
}

fn draw_line(ctx: &mut Context<'_>, x0: f64, y0: f64, x1: f64, y1: f64, color: Color) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let steps = (dx.abs().max(dy.abs()) * 2.0).ceil() as usize;
    let steps = steps.max(1);
    for step in 0..=steps {
        let t = step as f64 / steps as f64;
        let x = x0 + dx * t;
        let y = y0 + dy * t;
        draw_point(ctx, x, y, color);
    }
}

fn dim_color(color: Color, factor: f32) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            ((r as f32) * factor).clamp(0.0, 255.0) as u8,
            ((g as f32) * factor).clamp(0.0, 255.0) as u8,
            ((b as f32) * factor).clamp(0.0, 255.0) as u8,
        ),
        Color::White => Color::Gray,
        Color::Cyan => Color::DarkGray,
        Color::Blue => Color::DarkGray,
        Color::Green => Color::DarkGray,
        Color::Yellow => Color::DarkGray,
        Color::Red => Color::DarkGray,
        _ => Color::DarkGray,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_braille_graph_creation() {
        let data = vec![10, 20, 30, 40, 50];
        let graph = BrailleGraph::new(&data);
        assert_eq!(graph.data.len(), 5);
    }
}
