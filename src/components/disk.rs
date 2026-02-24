use crate::{
    action::Action,
    components::Component,
    data::snapshot::{DiskInfo, SystemSnapshot},
    theme::Theme,
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph},
    Frame,
};
use std::collections::HashMap;

pub struct DiskComponent {
    pub snapshot: SystemSnapshot,
    pub theme: Theme,
    pub selected_index: usize,
}

impl DiskComponent {
    pub fn new(snapshot: SystemSnapshot, theme: Theme) -> Self {
        Self {
            snapshot,
            theme,
            selected_index: 0,
        }
    }

    pub fn render_in_area(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let block = Block::default()
            .title(Span::styled(
                " Disk ",
                Style::default()
                    .fg(self.theme.get_color(Color::LightMagenta))
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(self.theme.get_color(Color::DarkGray)))
            .padding(ratatui::widgets::Padding::uniform(1));
        f.render_widget(&block, area);

        let inner_area = block.inner(area);

        let disks = Self::deduplicate_disks(&self.snapshot.disks);

        // Calculate overall disk usage
        let total_used: u64 = disks
            .iter()
            .map(|disk| disk.total_space - disk.available_space)
            .sum();
        let total_space: u64 = disks.iter().map(|disk| disk.total_space).sum();

        let total_percent = if total_space > 0 {
            (total_used as f64 / total_space as f64) * 100.0
        } else {
            0.0
        };

        // Split the inner area into summary (top) and list (bottom)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Overall summary
                Constraint::Min(0),    // Disk list
            ])
            .split(inner_area);

        // Render overall summary
        let summary_spans = vec![
            Line::from(vec![
                Span::styled(
                    "Total: ",
                    Style::default().fg(self.theme.get_color(Color::White)),
                ),
                Span::styled(
                    format!(
                        "{}/{}",
                        crate::utils::bytes_to_human_readable(total_used),
                        crate::utils::bytes_to_human_readable(total_space)
                    ),
                    Style::default().fg(self.theme.get_color(Color::Green)),
                ),
                Span::raw("  "),
                Span::styled(
                    format!("{:.1}%", total_percent),
                    Style::default().fg(self.theme.get_color(Color::Yellow)),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "Volumes: ",
                    Style::default().fg(self.theme.get_color(Color::White)),
                ),
                Span::styled(
                    format!("{}", disks.len()),
                    Style::default().fg(self.theme.get_color(Color::Cyan)),
                ),
            ]),
        ];

        let summary_paragraph = Paragraph::new(summary_spans).block(Block::default());
        f.render_widget(summary_paragraph, chunks[0]);

        // Render disk list (simplified)
        let disk_items: Vec<ListItem> = disks
            .iter()
            .enumerate()
            .map(|(i, disk)| {
                let used_space = disk.total_space - disk.available_space;
                let usage_percent = if disk.total_space > 0 {
                    (used_space as f64 / disk.total_space as f64) * 100.0
                } else {
                    0.0
                };

                let disk_line = format!(
                    "{}  {:.1}%  {}/{}",
                    disk.name,
                    usage_percent,
                    crate::utils::bytes_to_human_readable(used_space),
                    crate::utils::bytes_to_human_readable(disk.total_space)
                );

                let style = if i == self.selected_index {
                    Style::default()
                        .bg(self.theme.get_color(Color::Blue))
                        .fg(self.theme.get_color(Color::White))
                } else {
                    Style::default().fg(self.theme.get_color(Color::Yellow))
                };

                ListItem::new(disk_line).style(style)
            })
            .collect();

        let disk_list = List::new(disk_items).block(Block::default()); // No borders
        f.render_widget(disk_list, chunks[1]);
    }

    fn deduplicate_disks(disks: &[DiskInfo]) -> Vec<DiskInfo> {
        let mut by_key: HashMap<(String, u64), DiskInfo> = HashMap::new();
        for disk in disks {
            let key = (disk.name.clone(), disk.total_space);
            by_key
                .entry(key)
                .and_modify(|entry| {
                    entry.available_space = entry.available_space.min(disk.available_space);
                })
                .or_insert_with(|| disk.clone());
        }

        let mut deduped: Vec<DiskInfo> = by_key.into_values().collect();
        deduped.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then_with(|| b.total_space.cmp(&a.total_space))
        });
        deduped
    }
}

impl Component for DiskComponent {
    fn handle_events(
        &mut self,
        _event: crossterm::event::Event,
    ) -> Result<Option<Action>, Box<dyn std::error::Error>> {
        // Disk component doesn't handle events directly
        Ok(None)
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>, Box<dyn std::error::Error>> {
        match action {
            Action::MoveUp => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            Action::MoveDown => {
                let disk_count = Self::deduplicate_disks(&self.snapshot.disks).len();
                if disk_count > 0 && self.selected_index < disk_count - 1 {
                    self.selected_index += 1;
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn render(&mut self, f: &mut Frame) {
        self.render_in_area(f, f.area());
    }
}
