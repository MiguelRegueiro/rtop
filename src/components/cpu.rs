use crate::{
    action::Action, components::Component, data::snapshot::SystemSnapshot, theme::Theme,
    widgets::braille_graph::BrailleGraph,
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

pub struct CpuComponent {
    pub snapshot: SystemSnapshot,
    pub theme: Theme,
}

impl CpuComponent {
    pub fn new(snapshot: SystemSnapshot, theme: Theme) -> Self {
        Self { snapshot, theme }
    }

    fn get_cpu_color(&self, cpu_usage: f32) -> Color {
        if cpu_usage <= 50.0 {
            self.theme.get_color(Color::Green)
        } else if cpu_usage <= 80.0 {
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

    fn get_frequency_color(&self, freq: f32) -> Color {
        // Define frequency thresholds for color coding with neutral/cool palette
        // Idle (<1000MHz): Dim Slate/Grey (#6272a4)
        // Base (1000-3000MHz): Soft Cyan/Blue (#8be9fd)
        // Boost (>3000MHz): Bright Electric Purple or White (#bd93f9 or #ffffff)

        if freq < 1000.0 {
            // Idle: Dim Slate/Grey
            Color::Rgb(98, 114, 164) // #6272a4
        } else if freq <= 3000.0 {
            // Base: Soft Cyan/Blue
            Color::Rgb(139, 233, 253) // #8be9fd
        } else {
            // Boost: Bright Electric Purple
            Color::Rgb(189, 147, 249) // #bd93f9
        }
    }

    fn bar_width_for_area(area_width: u16, prefix_chars: usize, max_width: usize) -> usize {
        let available = area_width as usize;
        let room_after_prefix = available.saturating_sub(prefix_chars);
        room_after_prefix.min(max_width)
    }

    fn cpu_temp_priority(label: &str) -> Option<u8> {
        let lower = label.to_ascii_lowercase();
        if lower.is_empty() {
            return Some(1);
        }
        if lower.contains("package id")
            || lower.contains("x86_pkg_temp")
            || lower.contains("cpu package")
            || lower.contains("physical id")
        {
            return Some(7);
        }
        if lower.contains("tdie") || lower.contains("tctl") || lower.contains("tcpu") {
            return Some(6);
        }
        if lower.contains("cpu") || lower.contains("package") {
            return Some(5);
        }
        if lower.contains("coretemp") || lower.starts_with("core ") || lower.starts_with("coretemp")
        {
            return Some(4);
        }
        if lower.contains("soc") {
            return Some(3);
        }
        None
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
        let cpu_usage = self.snapshot.global_cpu_usage;
        let cpu_cores = self.snapshot.cpu_count;

        let cpu_title = if !self.snapshot.cpu_name.is_empty() {
            format!(" CPU · {} ", self.snapshot.cpu_name)
        } else {
            " CPU ".to_string()
        };

        let block = Block::default()
            .title(Span::styled(
                cpu_title,
                Style::default()
                    .fg(self.theme.get_color(Color::LightBlue))
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(self.theme.get_color(Color::DarkGray)))
            .padding(ratatui::widgets::Padding::uniform(1));
        f.render_widget(&block, area);

        let inner_area = block.inner(area);

        // Split the inner area into two parts: top section (global stats, bar, temp) and bottom section (core grid)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // Global CPU stats, bar, temp, sparkline
                Constraint::Min(0),    // Core Grid
            ])
            .split(inner_area);

        let top_section_area = chunks[0];
        let core_grid_area = chunks[1];

        let top_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Usage, Cores, Freq
                Constraint::Length(1), // Global CPU LineGauge
                Constraint::Length(1), // Temperature
                Constraint::Length(1), // Sparkline
            ])
            .split(top_section_area);

        let global_stats_area = top_chunks[0];
        let global_gauge_area = top_chunks[1];
        let temp_area = top_chunks[2];
        let sparkline_area = top_chunks[3];

        // Render global stats in the global_stats_area
        let mut total_stats_spans = vec![Line::from(vec![
            Span::styled("Usage: ", self.theme.text_style()),
            Span::styled(
                format!("{:.1}%", cpu_usage),
                Style::default().fg(self.get_cpu_color(cpu_usage)),
            ),
            Span::raw("   "), // Separator
            Span::styled("Cores: ", self.theme.text_style()),
            Span::styled(format!("{}", cpu_cores), self.theme.text_style()),
        ])];

        if !self.snapshot.cpu_frequencies.is_empty() {
            let avg_freq = self.snapshot.cpu_frequencies.iter().sum::<u64>() as f32
                / self.snapshot.cpu_frequencies.len() as f32;
            total_stats_spans[0].spans.push(Span::raw("   ")); // Separator
            total_stats_spans[0]
                .spans
                .push(Span::styled("Freq:  ", self.theme.text_style()));
            total_stats_spans[0].spans.push(Span::styled(
                format!("{:.0}MHz", avg_freq),
                self.theme.text_style(),
            ));
        }

        f.render_widget(Paragraph::new(total_stats_spans), global_stats_area);

        // Use fractional block characters for global CPU usage visualization with smooth gradient
        let usage_percentage = cpu_usage.min(100.0);
        let usage_prefix = format!("CPU:{:>6.1}% ", cpu_usage);
        let bar_width = Self::bar_width_for_area(global_gauge_area.width, usage_prefix.len(), 48);
        let total_units = bar_width * 8; // 8 sub-units per block
        let filled_units = (usage_percentage / 100.0 * total_units as f32).round() as usize;

        let mut spans = Vec::with_capacity(bar_width + 2); // label + bar cells
        spans.push(Span::styled(usage_prefix, self.theme.text_style()));

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
        let gauge_line = Line::from(spans);
        let paragraph = Paragraph::new(gauge_line);
        f.render_widget(paragraph, global_gauge_area);

        // Render Temperature and Power
        let mut temp_spans = Vec::new();

        // Add temperature
        let cpu_temp = self
            .snapshot
            .temperature_sensors
            .iter()
            .filter_map(|sensor| {
                Self::cpu_temp_priority(&sensor.label)
                    .map(|priority| (priority, sensor.temperature))
            })
            .max_by_key(|(priority, _)| *priority)
            .map(|(_, temperature)| temperature);
        if let Some(temperature) = cpu_temp {
            let temp_color = self.get_temperature_color(temperature);
            temp_spans.push(Span::styled(
                format!("Temp: {:.1}°C", temperature),
                Style::default().fg(temp_color),
            ));
        } else {
            temp_spans.push(Span::styled("Temp: N/A", self.theme.text_style()));
        }

        // Add power consumption
        if let Some(power) = self.snapshot.cpu_power {
            if power > 0.0 {
                // Only show if power is a real positive value, not N/A
                let power_color = self.get_temperature_color(power); // Use same color gradient as temperature
                temp_spans.push(Span::styled(" | ", self.theme.text_style())); // Separator
                temp_spans.push(Span::styled(
                    format!("Power: {:.1}W", power),
                    Style::default().fg(power_color),
                ));
            }
        }

        f.render_widget(Paragraph::new(Line::from(temp_spans)), temp_area);

        // Render overall CPU Braille graph (replacing sparkline)
        if !self.snapshot.cpu_history.is_empty() {
            let mut avg_cpu_data: Vec<u64> = Vec::new();
            if self.snapshot.cpu_count > 0 {
                // Assuming all cpu_history VecDeques are of similar length (e.g., 50)
                let history_len = self.snapshot.cpu_history.get(0).map_or(0, |h| h.len());

                for i in 0..history_len {
                    let mut sum = 0.0f32;
                    let mut count = 0;

                    for core_history in &self.snapshot.cpu_history {
                        if let Some(val) = core_history.get(i) {
                            sum += val;
                            count += 1;
                        }
                    }

                    if count > 0 {
                        avg_cpu_data.push((sum / count as f32) as u64);
                    }
                }
            }

            // Convert to percentage values (0-100) for the Braille graph
            let percentage_data: Vec<u64> = avg_cpu_data
                .iter()
                .map(|&val| (val as f64).round() as u64)
                .collect();

            let braille_graph = BrailleGraph::new(&percentage_data)
                .block(Block::default().borders(Borders::NONE)) // Remove borders for compactness
                .style(Style::default().fg(self.get_cpu_color(self.snapshot.global_cpu_usage)))
                .value_range(0.0, 100.0)
                .smoothing(2)
                .show_baseline(true)
                .use_gradient(true)
                .fill(false);
            f.render_widget(braille_graph, sparkline_area);
        } else {
            // Show a simple indicator if no history data
            f.render_widget(
                Paragraph::new(Span::styled("No history", self.theme.text_style())),
                sparkline_area,
            );
        }
        // Render per-core grid in core_grid_area
        let core_width = 18; // Approximate width needed for "0: [==> ] 10% 1234MHz"
        let num_cols = core_grid_area.width / core_width;

        if num_cols == 0 {
            // Handle case where there's no space for columns
            f.render_widget(
                Paragraph::new(Span::styled(
                    "Not enough space for core grid",
                    self.theme.text_style(),
                )),
                core_grid_area,
            );
            return;
        }
        let num_rows_per_col = (self.snapshot.cpu_count as f32 / num_cols as f32).ceil() as usize;

        let mut constraints = vec![];
        for _ in 0..num_cols {
            constraints.push(Constraint::Percentage(100 / num_cols));
        }

        let col_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(core_grid_area);

        for (col_index, col_chunk) in col_chunks.iter().enumerate() {
            let mut core_constraints = vec![];
            for _ in 0..num_rows_per_col {
                core_constraints.push(Constraint::Length(1)); // One line per core
            }
            let row_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(core_constraints)
                .split(*col_chunk);

            for row_index in 0..num_rows_per_col {
                let core_idx = col_index * num_rows_per_col + row_index;
                if core_idx < self.snapshot.cpu_count {
                    let core_usage = if !self.snapshot.cpu_history.is_empty()
                        && core_idx < self.snapshot.cpu_history.len()
                    {
                        *self.snapshot.cpu_history[core_idx].back().unwrap_or(&0.0)
                    } else {
                        0.0
                    };
                    let core_freq = if !self.snapshot.cpu_frequencies.is_empty()
                        && core_idx < self.snapshot.cpu_frequencies.len()
                    {
                        self.snapshot.cpu_frequencies[core_idx]
                    } else {
                        0
                    };

                    // Show only text for per-core usage (modern minimalist approach)
                    let core_text = format!("{:>2}: {:.1}% ", core_idx, core_usage);

                    // Apply color gradient to frequency based on boost levels
                    let freq_color = self.get_frequency_color(core_freq as f32);
                    let freq_span =
                        Span::styled(format!("{}MHz", core_freq), Style::default().fg(freq_color));

                    let mut spans = vec![Span::styled(core_text, self.theme.text_style())];
                    spans.push(freq_span);

                    let paragraph = Paragraph::new(Line::from(spans));
                    f.render_widget(paragraph, row_chunks[row_index]);
                }
            }
        }
    }
}

impl Component for CpuComponent {
    fn handle_events(
        &mut self,
        _event: crossterm::event::Event,
    ) -> Result<Option<Action>, Box<dyn std::error::Error>> {
        // CPU component doesn't handle events directly
        Ok(None)
    }

    fn update(&mut self, _action: Action) -> Result<Option<Action>, Box<dyn std::error::Error>> {
        // CPU component doesn't handle actions directly
        Ok(None)
    }

    fn render(&mut self, f: &mut Frame) {
        self.render_in_area(f, f.area());
    }
}
