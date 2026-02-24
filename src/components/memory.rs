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

pub struct MemoryComponent {
    pub snapshot: SystemSnapshot,
    pub theme: Theme,
}

impl MemoryComponent {
    pub fn new(snapshot: SystemSnapshot, theme: Theme) -> Self {
        Self { snapshot, theme }
    }

    fn get_memory_color(&self, memory_usage: f64) -> Color {
        if memory_usage <= 50.0 {
            self.theme.get_color(Color::Green)
        } else if memory_usage <= 80.0 {
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

    fn calculate_cached_color(&self, position_ratio: f32) -> Color {
        // Keep cached RAM in the same palette as used RAM, just dimmed.
        match self.calculate_gradient_color(position_ratio) {
            Color::Rgb(r, g, b) => Color::Rgb(
                (r as f32 * 0.7) as u8,
                (g as f32 * 0.7) as u8,
                (b as f32 * 0.7) as u8,
            ),
            _ => self.theme.get_color(Color::Yellow),
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
        let used_memory = self.snapshot.used_memory;
        let total_memory = self.snapshot.total_memory;
        let used_swap = self.snapshot.used_swap;
        let total_swap = self.snapshot.total_swap;

        let block = Block::default()
            .title(Span::styled(
                " Memory ",
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

        // Split inner area into top (bars) and bottom (sparkline)
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4), // RAM + SWAP bars
                Constraint::Min(0),    // Sparkline
            ])
            .split(inner_area);

        let bars_area = main_chunks[0];
        let sparkline_area = main_chunks[1];

        // Split bars area into RAM and SWAP sections
        let bar_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // RAM
                Constraint::Length(2), // SWAP
            ])
            .split(bars_area);

        let ram_area = bar_chunks[0];
        let swap_area = bar_chunks[1];

        // Render RAM with stacked bar for Used and Cached memory
        let cached_memory = self.snapshot.cached_memory;
        let used_memory_actual = used_memory.saturating_sub(cached_memory); // Actual used memory excluding cache
        let total_used_and_cached = used_memory.min(total_memory); // Total used + cached, capped at total

        let ram_label = format!(
            "RAM: {}/{}/{}",
            Self::bytes_to_human_readable(used_memory_actual),
            Self::bytes_to_human_readable(cached_memory),
            Self::bytes_to_human_readable(total_memory)
        );
        let ram_label_len = ram_label.len();

        let mut spans = Vec::new();
        spans.push(Span::styled(ram_label, self.theme.text_style()));

        // Create stacked bar visualization with consistent gradient style
        let ram_bar_width = Self::bar_width_for_area(ram_area.width, ram_label_len, 42);
        let total_units = ram_bar_width * 8; // 8 sub-units per block

        // Draw the stacked bar
        let used_ratio = if total_memory > 0 {
            used_memory_actual as f64 / total_memory as f64
        } else {
            0.0
        };
        let cached_ratio = if total_memory > 0 {
            cached_memory as f64 / total_memory as f64
        } else {
            0.0
        };

        let used_units = (used_ratio * total_units as f64) as usize;
        let cached_units = (cached_ratio * total_units as f64) as usize;

        for i in 0..ram_bar_width {
            let start_unit = i * 8;
            let end_unit = start_unit + 8;

            // Determine what type of memory is in this segment
            if start_unit >= used_units + cached_units {
                // Empty space - use track color based on position
                let position_ratio = i as f32 / (ram_bar_width.saturating_sub(1).max(1)) as f32;
                let track_color = self.calculate_track_color(position_ratio);
                spans.push(Span::styled(
                    ' '.to_string(),
                    Style::default().bg(track_color),
                ));
            } else if start_unit >= used_units {
                // Cached memory segment
                let is_partial = end_unit > used_units + cached_units;
                let position_ratio = i as f32 / (ram_bar_width.saturating_sub(1).max(1)) as f32;
                let track_color = self.calculate_track_color(position_ratio);
                let cached_color = self.calculate_cached_color(position_ratio);
                let char = if !is_partial {
                    ' ' // Fully filled with cached
                } else {
                    // Partially filled - determine the fractional character
                    let partial_units = (used_units + cached_units).saturating_sub(start_unit);
                    Self::fractional_block(partial_units)
                };

                let mut style = Style::default().bg(cached_color);
                if is_partial {
                    style = Style::default().fg(cached_color).bg(track_color);
                }
                spans.push(Span::styled(char.to_string(), style));
            } else {
                // Used memory segment
                let is_partial = end_unit > used_units;
                let position_ratio = i as f32 / (ram_bar_width.saturating_sub(1).max(1)) as f32;
                let track_color = self.calculate_track_color(position_ratio);
                let used_color = self.calculate_gradient_color(position_ratio);
                let char = if !is_partial {
                    ' ' // Fully filled with used
                } else {
                    // Partially filled - determine the fractional character
                    let partial_units = used_units.saturating_sub(start_unit);
                    Self::fractional_block(partial_units)
                };

                let mut style = Style::default().bg(used_color);
                if is_partial {
                    style = Style::default().fg(used_color).bg(track_color);
                }
                spans.push(Span::styled(char.to_string(), style));
            }
        }
        let ram_line = Line::from(spans);
        let ram_paragraph = Paragraph::new(ram_line);
        f.render_widget(ram_paragraph, ram_area);

        // Render SWAP
        let swap_percent = if total_swap > 0 {
            (used_swap as f64 / total_swap as f64) * 100.0
        } else {
            0.0
        };
        let swap_label = format!(
            "SWAP: {}/{} ({:.1}%)",
            Self::bytes_to_human_readable(used_swap),
            Self::bytes_to_human_readable(total_swap),
            swap_percent
        );
        // Use fractional block characters for SWAP usage visualization with smooth gradient
        let swap_usage_percentage = swap_percent.min(100.0);
        let swap_bar_width = Self::bar_width_for_area(swap_area.width, swap_label.len(), 42);
        let total_units = swap_bar_width * 8; // 8 sub-units per block
        let filled_units = (swap_usage_percentage / 100.0 * total_units as f64).round() as usize;

        let mut spans = Vec::new();
        spans.push(Span::styled(swap_label, self.theme.text_style()));

        for i in 0..swap_bar_width {
            let start_unit = i * 8;
            let end_unit = start_unit + 8;

            let position_ratio = i as f32 / (swap_bar_width.saturating_sub(1).max(1)) as f32;
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
        let swap_line = Line::from(spans);
        let swap_paragraph = Paragraph::new(swap_line);
        f.render_widget(swap_paragraph, swap_area);

        // Render memory Braille graph
        if !self.snapshot.memory_history.is_empty() {
            let mut mem_history_data: Vec<u64> = Vec::new();
            // Assuming all history VecDeques are of similar length (e.g., 50)
            let history_len = self.snapshot.memory_history.len();

            for i in 0..history_len {
                if let Some((used, total)) = self.snapshot.memory_history.get(i) {
                    let percent = if *total > 0 {
                        (*used as f64 / *total as f64) * 100.0
                    } else {
                        0.0
                    };
                    // Keep one decimal point precision for a smoother graph line.
                    mem_history_data.push((percent * 10.0).round() as u64);
                }
            }

            let ram_percent = if total_memory > 0 {
                (total_used_and_cached as f64 / total_memory as f64) * 100.0
            } else {
                0.0
            };

            let braille_graph = BrailleGraph::new(&mem_history_data)
                .block(Block::default().borders(Borders::NONE))
                .style(Style::default().fg(self.get_memory_color(ram_percent))) // Use RAM percent for graph color
                .value_range(0.0, 1000.0)
                .smoothing(3)
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
    }

    fn bytes_to_human_readable(bytes: u64) -> String {
        const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
        let mut size = bytes as f64;
        let mut unit_idx = 0;

        while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
            size /= 1024.0;
            unit_idx += 1;
        }

        format!("{:.1}{}", size, UNITS[unit_idx])
    }
}

impl Component for MemoryComponent {
    fn handle_events(
        &mut self,
        _event: crossterm::event::Event,
    ) -> Result<Option<Action>, Box<dyn std::error::Error>> {
        // Memory component doesn't handle events directly
        Ok(None)
    }

    fn update(&mut self, _action: Action) -> Result<Option<Action>, Box<dyn std::error::Error>> {
        // Memory component doesn't handle actions directly
        Ok(None)
    }

    fn render(&mut self, f: &mut Frame) {
        self.render_in_area(f, f.area());
    }
}
