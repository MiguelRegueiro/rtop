use crate::{action::Action, components::Component, data::snapshot::SystemSnapshot, theme::Theme};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Axis, Block, BorderType, Borders, Chart, Dataset, GraphType, Paragraph},
    Frame,
};

pub struct NetworkComponent {
    pub snapshot: SystemSnapshot,
    pub theme: Theme,
    pub show_graphs: bool,
}

impl NetworkComponent {
    pub fn new(snapshot: SystemSnapshot, theme: Theme) -> Self {
        Self {
            snapshot,
            theme,
            show_graphs: true,
        }
    }

    pub fn render_in_area(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let block = Block::default()
            .title(Span::styled(
                " Network ",
                Style::default()
                    .fg(self.theme.get_color(Color::LightCyan))
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(self.theme.get_color(Color::DarkGray)));
        f.render_widget(&block, area);

        let inner_area = block.inner(area);

        // Split the inner area into two parts: summary (top) and chart (bottom)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Summary lines
                Constraint::Min(0),    // Chart
            ])
            .split(inner_area);

        let selected_iface = self.snapshot.selected_network_interface.as_deref();
        let (current_rx_rate, current_tx_rate, total_rx, total_tx) = match selected_iface {
            Some(interface) => self
                .snapshot
                .networks
                .iter()
                .find(|net| net.name == interface)
                .map(|net| {
                    (
                        net.received_per_sec,
                        net.transmitted_per_sec,
                        net.total_received,
                        net.total_transmitted,
                    )
                })
                .unwrap_or((0, 0, 0, 0)),
            None => (
                self.snapshot
                    .networks
                    .iter()
                    .map(|net| net.received_per_sec)
                    .sum(),
                self.snapshot
                    .networks
                    .iter()
                    .map(|net| net.transmitted_per_sec)
                    .sum(),
                self.snapshot
                    .networks
                    .iter()
                    .map(|net| net.total_received)
                    .sum(),
                self.snapshot
                    .networks
                    .iter()
                    .map(|net| net.total_transmitted)
                    .sum(),
            ),
        };

        // Render summary
        let summary_spans = vec![
            Line::from(vec![
                Span::styled(
                    "RX: ",
                    Style::default().fg(self.theme.get_color(Color::White)),
                ),
                Span::styled(
                    format!(
                        "{}/s",
                        crate::utils::bytes_to_human_readable(current_rx_rate)
                    ),
                    Style::default().fg(self.theme.get_color(Color::Green)),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "TX: ",
                    Style::default().fg(self.theme.get_color(Color::White)),
                ),
                Span::styled(
                    format!(
                        "{}/s",
                        crate::utils::bytes_to_human_readable(current_tx_rate)
                    ),
                    Style::default().fg(self.theme.get_color(Color::Red)),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "Interface: ",
                    Style::default().fg(self.theme.get_color(Color::White)),
                ),
                Span::styled(
                    self.snapshot
                        .selected_network_interface
                        .clone()
                        .unwrap_or_else(|| "All".to_string()),
                    Style::default().fg(self.theme.get_color(Color::Blue)),
                ),
                Span::raw("  "),
                Span::styled(
                    format!(
                        "{} / {}",
                        crate::utils::bytes_to_human_readable(total_rx),
                        crate::utils::bytes_to_human_readable(total_tx)
                    ),
                    Style::default().fg(self.theme.get_color(Color::Gray)),
                ),
            ]),
        ];

        let summary_paragraph = Paragraph::new(summary_spans).block(Block::default());
        f.render_widget(summary_paragraph, chunks[0]);

        // Render chart
        if self.show_graphs && !self.snapshot.network_history.is_empty() {
            // Prepare data for RX and TX
            let rx_data_raw: Vec<(f64, f64)> = self
                .snapshot
                .network_history
                .iter()
                .enumerate()
                .map(|(i, &(rx, _))| (i as f64, rx as f64))
                .collect();

            let tx_data_raw: Vec<(f64, f64)> = self
                .snapshot
                .network_history
                .iter()
                .enumerate()
                .map(|(i, &(_, tx))| (i as f64, tx as f64))
                .collect();

            let rx_data = Self::smooth_series(&rx_data_raw, 2);
            let tx_data = Self::smooth_series(&tx_data_raw, 2);

            let rx_dataset = Dataset::default()
                .name("RX")
                .data(&rx_data)
                .graph_type(GraphType::Line)
                .style(
                    Style::default()
                        .fg(self.theme.get_color(Color::Cyan))
                        .add_modifier(ratatui::style::Modifier::BOLD),
                );

            let tx_dataset = Dataset::default()
                .name("TX")
                .data(&tx_data)
                .graph_type(GraphType::Line)
                .style(
                    Style::default()
                        .fg(self.theme.get_color(Color::LightBlue))
                        .add_modifier(ratatui::style::Modifier::BOLD),
                );

            let max_y = rx_data
                .iter()
                .chain(tx_data.iter())
                .map(|(_, value)| *value)
                .fold(0.0_f64, f64::max)
                .max(1.0);

            let x_max = (self.snapshot.network_history.len().saturating_sub(1)) as f64;
            let x_bound = x_max.max(1.0);
            let x_mid = (x_bound / 2.0).round();

            let x_axis = Axis::default()
                .bounds([0.0, x_bound])
                .style(Style::default().fg(self.theme.get_color(Color::DarkGray)))
                .labels(vec![
                    Span::styled("0", Style::default().fg(self.theme.get_color(Color::Gray))),
                    Span::styled(
                        format!("{:.0}", x_mid),
                        Style::default().fg(self.theme.get_color(Color::Gray)),
                    ),
                    Span::styled(
                        format!("{:.0}", x_bound),
                        Style::default().fg(self.theme.get_color(Color::Gray)),
                    ),
                ]);

            let y_max_bound = Self::nice_axis_upper(max_y * 1.15);
            let y_mid_bound = y_max_bound / 2.0;
            let y_axis = Axis::default()
                .bounds([0.0, y_max_bound])
                .style(Style::default().fg(self.theme.get_color(Color::DarkGray)))
                .labels(vec![
                    Span::styled("0", Style::default().fg(self.theme.get_color(Color::Gray))),
                    Span::styled(
                        crate::utils::bytes_to_human_readable(y_mid_bound as u64),
                        Style::default().fg(self.theme.get_color(Color::Gray)),
                    ),
                    Span::styled(
                        crate::utils::bytes_to_human_readable(y_max_bound as u64),
                        Style::default().fg(self.theme.get_color(Color::Gray)),
                    ),
                ]);

            let chart = Chart::new(vec![rx_dataset, tx_dataset])
                .block(
                    Block::default()
                        .borders(Borders::TOP)
                        .border_style(Style::default().fg(self.theme.get_color(Color::DarkGray))),
                )
                .x_axis(x_axis)
                .y_axis(y_axis);

            f.render_widget(chart, chunks[1]);
        } else if !self.show_graphs {
            let info_block = Paragraph::new("Graphs disabled")
                .block(Block::default())
                .style(Style::default().fg(self.theme.get_color(Color::Gray)));
            f.render_widget(info_block, chunks[1]);
        }
    }

    fn smooth_series(data: &[(f64, f64)], radius: usize) -> Vec<(f64, f64)> {
        if data.len() < 3 || radius == 0 {
            return data.to_vec();
        }

        let mut out = Vec::with_capacity(data.len());
        for (idx, (x, _)) in data.iter().enumerate() {
            let start = idx.saturating_sub(radius);
            let end = (idx + radius + 1).min(data.len());
            let window = &data[start..end];
            let avg = window.iter().map(|(_, y)| *y).sum::<f64>() / window.len() as f64;
            out.push((*x, avg));
        }
        out
    }

    fn nice_axis_upper(value: f64) -> f64 {
        if value <= 1.0 {
            return 1.0;
        }

        let magnitude = 10_f64.powf(value.log10().floor());
        let normalized = value / magnitude;
        let step = if normalized <= 1.0 {
            1.0
        } else if normalized <= 2.0 {
            2.0
        } else if normalized <= 5.0 {
            5.0
        } else {
            10.0
        };
        step * magnitude
    }
}

impl Component for NetworkComponent {
    fn handle_events(
        &mut self,
        _event: crossterm::event::Event,
    ) -> Result<Option<Action>, Box<dyn std::error::Error>> {
        // Network component doesn't handle events directly
        Ok(None)
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>, Box<dyn std::error::Error>> {
        match action {
            Action::ToggleGraphs => {
                self.show_graphs = !self.show_graphs;
            }
            _ => {}
        }
        Ok(None)
    }

    fn render(&mut self, f: &mut Frame) {
        self.render_in_area(f, f.area());
    }
}
