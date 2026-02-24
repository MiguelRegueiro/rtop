use crate::{action::Action, components::Component, data::snapshot::SystemSnapshot, theme::Theme};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

pub struct GpuComponent {
    pub snapshot: SystemSnapshot,
    pub theme: Theme,
}

impl GpuComponent {
    pub fn new(snapshot: SystemSnapshot, theme: Theme) -> Self {
        Self { snapshot, theme }
    }

    #[allow(dead_code)]
    fn get_gpu_color(&self, usage: f32) -> Color {
        if usage <= 50.0 {
            self.theme.get_color(Color::Green)
        } else if usage <= 80.0 {
            self.theme.get_color(Color::Yellow)
        } else {
            self.theme.get_color(Color::Red)
        }
    }

    fn calculate_gradient_color(&self, position_ratio: f32) -> Color {
        // Smooth gradient using hex colors with LERP
        // 0%->50%: Blend from #00ff87 to #f9ff00
        // 50%->100%: Blend from #f9ff00 to #ff003c
        if position_ratio < 0.5 {
            // From #00ff87 to #f9ff00 (0.0 to 0.5)
            let t = position_ratio * 2.0; // Normalize to 0.0-1.0 range
                                          // Interpolate between #00ff87 (0, 255, 135) and #f9ff00 (249, 255, 0)
            let r = (0.0 + (249.0 - 0.0) * t) as u8;
            let g = (255.0 + (255.0 - 255.0) * t) as u8;
            let b = (135.0 + (0.0 - 135.0) * t) as u8;
            Color::Rgb(r, g, b)
        } else {
            // From #f9ff00 to #ff003c (0.5 to 1.0)
            let t = (position_ratio - 0.5) * 2.0; // Normalize to 0.0-1.0 range
                                                  // Interpolate between #f9ff00 (249, 255, 0) and #ff003c (255, 0, 60)
            let r = (249.0 + (255.0 - 249.0) * t) as u8;
            let g = (255.0 + (0.0 - 255.0) * t) as u8;
            let b = (0.0 + (60.0 - 0.0) * t) as u8;
            Color::Rgb(r, g, b)
        }
    }

    fn calculate_track_color(&self, position_ratio: f32) -> Color {
        // Dimmed version of the gradient color for the track (30% brightness)
        let active_color = self.calculate_gradient_color(position_ratio);
        match active_color {
            Color::Rgb(r, g, b) => {
                // Reduce brightness to 30% for visible track
                Color::Rgb(
                    (r as f32 * 0.3) as u8,
                    (g as f32 * 0.3) as u8,
                    (b as f32 * 0.3) as u8,
                )
            }
            _ => Color::Rgb(50, 50, 50), // Fallback to medium grey
        }
    }

    fn get_temperature_color(&self, temp: f32) -> Color {
        // Dynamic temperature color based on LERP with proper thresholds
        // Only turn 'Neon Rose' color when it actually crosses dangerous threshold (85°C+)
        if temp < 50.0 {
            // Cool: Electric Emerald (#00ff87)
            Color::Rgb(0, 255, 135)
        } else if temp <= 75.0 {
            // Moderate: Interpolate between Electric Emerald and Cyber Yellow
            let t = (temp - 50.0) / 25.0; // Normalize to 0.0-1.0 range
            let r = (0.0 + (249.0 - 0.0) * t) as u8;
            let g = (255.0 + (255.0 - 255.0) * t) as u8;
            let b = (135.0 + (0.0 - 135.0) * t) as u8;
            Color::Rgb(r, g, b)
        } else if temp <= 85.0 {
            // Warm: Interpolate between Cyber Yellow and Orange
            let t = (temp - 75.0) / 10.0; // Normalize to 0.0-1.0 range
            let r = (249.0 + (255.0 - 249.0) * t) as u8; // From 249 to 255 (red)
            let g = (255.0 + (165.0 - 255.0) * t) as u8; // From 255 to 165 (green)
            let b = (0.0 + (0.0 - 0.0) * t) as u8; // Stay at 0 (blue)
            Color::Rgb(r, g, b) // Approaching orange: #FFA500
        } else {
            // Dangerous: Neon Rose (#ff003c) for temperatures above 85°C
            Color::Rgb(255, 0, 60)
        }
    }

    fn bar_width_for_area(area_width: u16, prefix_chars: usize, max_width: usize) -> usize {
        let available = area_width as usize;
        let room_after_prefix = available.saturating_sub(prefix_chars);
        room_after_prefix.min(max_width)
    }

    fn fractional_block(units: usize) -> char {
        match units {
            1 => '▏',
            2 => '▎',
            3 => '▍',
            4 => '▌',
            5 => '▋',
            6 => '▊',
            7 => '▉',
            _ => '░',
        }
    }

    pub fn render_in_area(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let block = Block::default()
            .title(Span::styled(
                " GPU ",
                self.theme
                    .text_style()
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(self.theme.get_color(Color::DarkGray)))
            .padding(ratatui::widgets::Padding::uniform(1));
        f.render_widget(&block, area);

        let inner_area = block.inner(area);

        if self.snapshot.gpus.is_empty() {
            f.render_widget(
                Paragraph::new(Span::styled("No GPUs detected.", self.theme.text_style())),
                inner_area,
            );
            return;
        }

        // Calculate height needed for each GPU - allowing more space for the new layout
        let mut gpu_heights = Vec::new();
        for _gpu_info in &self.snapshot.gpus {
            // Fixed height for the new layout (name, bar, and stats)
            let height = 4; // Height for name, bar, and stats grid

            gpu_heights.push(height);
        }

        let gpu_constraints: Vec<Constraint> =
            gpu_heights.iter().map(|&h| Constraint::Length(h)).collect();
        let gpu_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(gpu_constraints)
            .margin(1) // Add margin between GPU blocks
            .split(inner_area);

        for (i, (gpu_info, &gpu_height)) in self
            .snapshot
            .gpus
            .iter()
            .zip(gpu_heights.iter())
            .enumerate()
        {
            if i >= gpu_chunks.len() {
                break;
            }

            let gpu_area = gpu_chunks[i];

            // Split GPU area into: Name, Bar, Stats
            let mut constraints = vec![
                Constraint::Length(1), // Name header
                Constraint::Length(1), // Usage bar
            ];

            // Add constraint for stats grid
            constraints.push(Constraint::Length(gpu_height - 2)); // Remaining space for stats

            let per_gpu_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints)
                .split(gpu_area);

            let name_area = per_gpu_chunks[0];
            let bar_area = per_gpu_chunks[1];
            let stats_area = per_gpu_chunks[2];

            // Render GPU Name Header
            let name_text = format!("{}", gpu_info.name);
            let name_paragraph = Paragraph::new(Span::styled(
                name_text,
                Style::default()
                    .fg(self.theme.get_color(Color::Cyan))
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ))
            .wrap(ratatui::widgets::Wrap { trim: true });
            f.render_widget(name_paragraph, name_area);

            // Render Hero Bar (Usage % and Gradient Bar)
            let usage_percentage = gpu_info.usage.map(|value| value.clamp(0.0, 100.0));
            let usage_prefix = match usage_percentage {
                Some(usage) => format!("Usage:{:>6.1}% ", usage),
                None => "Usage:  N/A  ".to_string(),
            };
            let bar_width = Self::bar_width_for_area(bar_area.width, usage_prefix.len(), 48);
            let total_units = bar_width * 8; // 8 sub-units per block
            let filled_units =
                (usage_percentage.unwrap_or(0.0) / 100.0 * total_units as f32).round() as usize;

            let mut spans = Vec::new();
            spans.push(Span::styled(
                usage_prefix,
                if usage_percentage.is_some() {
                    self.theme.text_style()
                } else {
                    Style::default().fg(self.theme.get_color(Color::Gray))
                },
            ));

            for i in 0..bar_width {
                let start_unit = i * 8;
                let end_unit = start_unit + 8;
                let position_ratio = i as f32 / (bar_width.saturating_sub(1).max(1)) as f32;
                let active_color = self.calculate_gradient_color(position_ratio);
                let track_color = self.calculate_track_color(position_ratio);

                let (char, style) = if filled_units <= start_unit {
                    (' ', Style::default().bg(track_color))
                } else if filled_units >= end_unit {
                    (' ', Style::default().bg(active_color))
                } else {
                    let partial_units = filled_units - start_unit;
                    (
                        Self::fractional_block(partial_units),
                        Style::default().fg(active_color).bg(track_color),
                    )
                };

                spans.push(Span::styled(char.to_string(), style));
            }
            let usage_line = Line::from(spans);
            let paragraph = Paragraph::new(usage_line);
            f.render_widget(paragraph, bar_area);

            // Render fixed compact stats with stable field widths.
            let temp_field = if let Some(temp) = gpu_info.temp {
                (
                    format!("T:{:.0}C", temp),
                    Style::default().fg(self.get_temperature_color(temp)),
                )
            } else {
                (
                    "T:--".to_string(),
                    Style::default().fg(self.theme.get_color(Color::Gray)),
                )
            };
            let power_field = if let Some(power) = gpu_info.power_usage {
                if power > 0.0 {
                    (
                        format!("P:{:.1}W", power),
                        Style::default().fg(self.theme.get_color(Color::Yellow)),
                    )
                } else {
                    (
                        "P:--".to_string(),
                        Style::default().fg(self.theme.get_color(Color::Gray)),
                    )
                }
            } else {
                (
                    "P:--".to_string(),
                    Style::default().fg(self.theme.get_color(Color::Gray)),
                )
            };
            let mem_field = if let (Some(mem_used), Some(mem_total)) =
                (gpu_info.memory_used, gpu_info.memory_total)
            {
                (
                    format!(
                        "M:{}/{}",
                        crate::utils::bytes_to_human_readable(mem_used),
                        crate::utils::bytes_to_human_readable(mem_total)
                    ),
                    self.theme.text_style(),
                )
            } else if let Some(mem_used) = gpu_info.memory_used {
                (
                    format!("M:{}", crate::utils::bytes_to_human_readable(mem_used)),
                    self.theme.text_style(),
                )
            } else if let Some(mem_total) = gpu_info.memory_total {
                (
                    format!("M:--/{}", crate::utils::bytes_to_human_readable(mem_total)),
                    Style::default().fg(self.theme.get_color(Color::Gray)),
                )
            } else {
                (
                    "M:--".to_string(),
                    Style::default().fg(self.theme.get_color(Color::Gray)),
                )
            };
            let stats_line = Line::from(vec![
                Span::styled(temp_field.0, temp_field.1),
                Span::raw("  "),
                Span::styled(power_field.0, power_field.1),
                Span::raw("  "),
                Span::styled(mem_field.0, mem_field.1),
            ]);
            f.render_widget(
                Paragraph::new(stats_line).wrap(ratatui::widgets::Wrap { trim: true }),
                stats_area,
            );
        }
    }

    // Use the shared utility function instead of duplicating code
}

impl Component for GpuComponent {
    fn handle_events(
        &mut self,
        _event: crossterm::event::Event,
    ) -> Result<Option<Action>, Box<dyn std::error::Error>> {
        Ok(None)
    }

    fn update(&mut self, _action: Action) -> Result<Option<Action>, Box<dyn std::error::Error>> {
        // GPU component doesn't handle actions directly
        Ok(None)
    }

    fn render(&mut self, f: &mut Frame) {
        self.render_in_area(f, f.area());
    }
}
