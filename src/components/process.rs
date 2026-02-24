use crate::{
    action::Action,
    components::Component,
    data::snapshot::{ProcessInfo, SystemSnapshot},
    theme::Theme,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};
use std::process::Command;

#[derive(Clone)]
struct ProcessRow {
    pid: u32,
    name: String,
    line: String,
}

#[derive(Clone)]
struct KillDialog {
    pid: u32,
    name: String,
    yes_selected: bool,
}

pub struct ProcessComponent {
    pub snapshot: SystemSnapshot,
    pub theme: Theme,
    pub selected_index: usize,
    pub show_tree: bool,
    filter_query: String,
    search_mode: bool,
    search_input: String,
    search_prev_filter: String,
    kill_dialog: Option<KillDialog>,
    status_message: Option<String>,
}

impl ProcessComponent {
    pub fn new(snapshot: SystemSnapshot, theme: Theme) -> Self {
        Self {
            snapshot,
            theme,
            selected_index: 0,
            show_tree: false,
            filter_query: String::new(),
            search_mode: false,
            search_input: String::new(),
            search_prev_filter: String::new(),
            kill_dialog: None,
            status_message: None,
        }
    }

    pub fn is_search_mode(&self) -> bool {
        self.search_mode
    }

    pub fn is_kill_confirm_active(&self) -> bool {
        self.kill_dialog.is_some()
    }

    fn current_filter(&self) -> &str {
        if self.search_mode {
            &self.search_input
        } else {
            &self.filter_query
        }
    }

    fn normalized_filter(&self) -> Option<String> {
        let raw = self.current_filter().trim();
        if raw.is_empty() {
            None
        } else {
            Some(raw.to_lowercase())
        }
    }

    fn process_matches_filter(process: &ProcessInfo, needle: Option<&str>) -> bool {
        let Some(needle) = needle else {
            return true;
        };
        if process.pid.to_string().contains(needle) || process.name.to_lowercase().contains(needle)
        {
            return true;
        }
        if process
            .cmd
            .iter()
            .any(|part| part.to_lowercase().contains(needle))
        {
            return true;
        }
        if let Some(exe) = &process.exe {
            if exe.to_lowercase().contains(needle) {
                return true;
            }
        }
        false
    }

    fn get_sorted_processes(&self) -> Vec<ProcessInfo> {
        let mut processes: Vec<ProcessInfo> = self.snapshot.processes.clone();

        match self.snapshot.process_sort_by {
            crate::data::snapshot::ProcessSortBy::CpuUsage => {
                processes.sort_by(|a, b| {
                    b.cpu_usage
                        .partial_cmp(&a.cpu_usage)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| b.memory.cmp(&a.memory))
                        .then_with(|| a.pid.cmp(&b.pid))
                });
            }
            crate::data::snapshot::ProcessSortBy::Memory => {
                processes.sort_by(|a, b| b.memory.cmp(&a.memory).then_with(|| a.pid.cmp(&b.pid)));
            }
            crate::data::snapshot::ProcessSortBy::Pid => {
                processes.sort_by(|a, b| b.pid.cmp(&a.pid));
            }
            crate::data::snapshot::ProcessSortBy::Name => {
                processes.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.pid.cmp(&b.pid)));
            }
        }

        processes
    }

    fn get_filtered_sorted_processes(&self) -> Vec<ProcessInfo> {
        let processes = self.get_sorted_processes();
        let filter = self.normalized_filter();
        processes
            .into_iter()
            .filter(|process| Self::process_matches_filter(process, filter.as_deref()))
            .collect()
    }

    fn get_flat_process_rows(&self) -> Vec<ProcessRow> {
        self.get_filtered_sorted_processes()
            .into_iter()
            .map(|process| ProcessRow {
                pid: process.pid,
                name: process.name.clone(),
                line: format!(
                    "{:>7} {:>8} {:>6.2}% {:>9} {}",
                    process.pid,
                    Self::bytes_to_human_readable(process.memory),
                    process.cpu_usage,
                    Self::bytes_to_human_readable(process.disk_usage),
                    process.name
                ),
            })
            .collect()
    }

    fn get_process_tree_rows(&self) -> Vec<ProcessRow> {
        let mut rows = Vec::new();
        let mut processes_map: std::collections::HashMap<u32, Vec<&ProcessInfo>> =
            std::collections::HashMap::new();

        for process in &self.snapshot.processes {
            let parent_pid = process.parent_pid.unwrap_or(0);
            processes_map.entry(parent_pid).or_default().push(process);
        }

        let filter = self.normalized_filter();
        Self::build_tree_recursive(0, &processes_map, 0, &mut rows, filter.as_deref());
        Self::build_tree_recursive(1, &processes_map, 0, &mut rows, filter.as_deref());
        rows
    }

    fn build_tree_recursive(
        parent_pid: u32,
        processes_map: &std::collections::HashMap<u32, Vec<&ProcessInfo>>,
        depth: usize,
        rows: &mut Vec<ProcessRow>,
        filter: Option<&str>,
    ) {
        if let Some(children) = processes_map.get(&parent_pid) {
            let mut ordered_children = children.clone();
            ordered_children.sort_by(|a, b| {
                b.cpu_usage
                    .partial_cmp(&a.cpu_usage)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.pid.cmp(&b.pid))
            });
            for process in ordered_children {
                if Self::process_matches_filter(process, filter) {
                    let indent = "  ".repeat(depth);
                    rows.push(ProcessRow {
                        pid: process.pid,
                        name: process.name.clone(),
                        line: format!(
                            "{}{} [{}] {:.2}% {}",
                            indent,
                            process.name,
                            process.pid,
                            process.cpu_usage,
                            Self::bytes_to_human_readable(process.memory)
                        ),
                    });
                }

                Self::build_tree_recursive(process.pid, processes_map, depth + 1, rows, filter);
            }
        }
    }

    fn get_process_rows(&self) -> Vec<ProcessRow> {
        if self.show_tree {
            self.get_process_tree_rows()
        } else {
            self.get_flat_process_rows()
        }
    }

    fn selected_row(&self) -> Option<ProcessRow> {
        let rows = self.get_process_rows();
        if rows.is_empty() {
            None
        } else {
            let idx = self.selected_index.min(rows.len().saturating_sub(1));
            rows.get(idx).cloned()
        }
    }

    fn clamp_selected_index(&mut self) {
        let count = self.get_process_rows().len();
        if count == 0 {
            self.selected_index = 0;
        } else if self.selected_index >= count {
            self.selected_index = count - 1;
        }
    }

    fn kill_process(&mut self, pid: u32, name: &str) {
        match Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .status()
        {
            Ok(status) if status.success() => {
                self.status_message = Some(format!("SIGTERM sent to {} ({})", name, pid));
            }
            Ok(status) => {
                self.status_message =
                    Some(format!("Failed to terminate PID {} (exit {})", pid, status));
            }
            Err(err) => {
                self.status_message = Some(format!("Failed to run kill for PID {}: {}", pid, err));
            }
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

    fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(area);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }
}

impl ProcessComponent {
    pub fn render_in_area(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let sort_label = match self.snapshot.process_sort_by {
            crate::data::snapshot::ProcessSortBy::CpuUsage => "CPU",
            crate::data::snapshot::ProcessSortBy::Memory => "MEM",
            crate::data::snapshot::ProcessSortBy::Pid => "PID",
            crate::data::snapshot::ProcessSortBy::Name => "NAME",
        };
        let mode_label = if self.show_tree { "tree" } else { "list" };

        let filter_suffix = if self.filter_query.trim().is_empty() {
            String::new()
        } else {
            format!(" · filter:{}", self.filter_query)
        };
        let title = format!(
            " Processes · {} · sort:{}{} ",
            mode_label, sort_label, filter_suffix
        );

        let rows = self.get_process_rows();

        let block = Block::default()
            .title(Span::styled(
                title,
                Style::default()
                    .fg(self.theme.get_color(Color::LightGreen))
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(self.theme.get_color(Color::DarkGray)));
        f.render_widget(&block, area);

        let inner = block.inner(area);
        let body_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(inner);

        let visible_rows = body_chunks[1].height as usize;
        let selected = if rows.is_empty() {
            0
        } else {
            self.selected_index.min(rows.len() - 1)
        };
        let window_start = if visible_rows > 0 && selected >= visible_rows {
            selected + 1 - visible_rows
        } else {
            0
        };
        let window_end = if visible_rows == 0 {
            0
        } else {
            (window_start + visible_rows).min(rows.len())
        };

        let mut header_line = if self.search_mode {
            format!(
                " search: {}_  [enter] apply [esc] cancel",
                self.search_input
            )
        } else if self.show_tree {
            format!(
                " tree entries: {}  selected: {}/{}",
                rows.len(),
                if rows.is_empty() { 0 } else { selected + 1 },
                rows.len()
            )
        } else {
            format!(
                " {:>7} {:>8} {:>6} {:>9}  {}   [{}/{}]",
                "PID",
                "MEM",
                "CPU%",
                "WRITE",
                "NAME",
                if rows.is_empty() { 0 } else { selected + 1 },
                rows.len()
            )
        };

        if let Some(msg) = &self.status_message {
            header_line.push_str("  ·  ");
            header_line.push_str(msg);
        }

        let header_style = if self.search_mode {
            Style::default().fg(self.theme.get_color(Color::Yellow))
        } else {
            Style::default().fg(self.theme.get_color(Color::Cyan))
        };

        let header = Paragraph::new(Line::from(Span::styled(header_line, header_style)))
            .wrap(Wrap { trim: true });
        f.render_widget(header, body_chunks[0]);

        let process_items: Vec<ListItem> = if rows.is_empty() {
            vec![ListItem::new("No processes")
                .style(Style::default().fg(self.theme.get_color(Color::Gray)))]
        } else {
            rows[window_start..window_end]
                .iter()
                .enumerate()
                .map(|(idx, row)| {
                    let global_index = window_start + idx;
                    let style = if global_index == selected {
                        Style::default()
                            .bg(self.theme.get_color(Color::Blue))
                            .fg(self.theme.get_color(Color::White))
                    } else {
                        Style::default().fg(self.theme.get_color(Color::Gray))
                    };
                    ListItem::new(row.line.clone()).style(style)
                })
                .collect()
        };
        let process_list = List::new(process_items);
        f.render_widget(process_list, body_chunks[1]);

        if let Some(dialog) = &self.kill_dialog {
            let popup_area = Self::centered_rect(66, 32, area);
            f.render_widget(Clear, popup_area);

            let yes_style = if dialog.yes_selected {
                Style::default()
                    .fg(self.theme.get_color(Color::Black))
                    .bg(self.theme.get_color(Color::Green))
                    .add_modifier(ratatui::style::Modifier::BOLD)
            } else {
                Style::default().fg(self.theme.get_color(Color::Gray))
            };
            let no_style = if dialog.yes_selected {
                Style::default().fg(self.theme.get_color(Color::Gray))
            } else {
                Style::default()
                    .fg(self.theme.get_color(Color::Black))
                    .bg(self.theme.get_color(Color::Red))
                    .add_modifier(ratatui::style::Modifier::BOLD)
            };

            let lines = vec![
                Line::from(Span::styled(
                    format!("Terminate '{}' (PID {})?", dialog.name, dialog.pid),
                    Style::default().fg(self.theme.get_color(Color::White)),
                )),
                Line::from(vec![
                    Span::styled(" ", self.theme.text_style()),
                    Span::styled(" Yes ", yes_style),
                    Span::styled("   ", self.theme.text_style()),
                    Span::styled(" No ", no_style),
                ]),
                Line::from(Span::styled(
                    "Enter: confirm  Esc: cancel",
                    Style::default().fg(self.theme.get_color(Color::DarkGray)),
                )),
            ];

            let dialog_widget = Paragraph::new(lines)
                .block(
                    Block::default()
                        .title(Span::styled(
                            " Confirm Termination ",
                            Style::default()
                                .fg(self.theme.get_color(Color::LightRed))
                                .add_modifier(ratatui::style::Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(self.theme.get_color(Color::DarkGray))),
                )
                .wrap(Wrap { trim: true });

            f.render_widget(dialog_widget, popup_area);
        }
    }
}

impl Component for ProcessComponent {
    fn handle_events(
        &mut self,
        _event: crossterm::event::Event,
    ) -> Result<Option<Action>, Box<dyn std::error::Error>> {
        Ok(None)
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>, Box<dyn std::error::Error>> {
        match action {
            Action::MoveUp => {
                if self.kill_dialog.is_none() && !self.search_mode && self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            Action::MoveDown => {
                if self.kill_dialog.is_none() && !self.search_mode {
                    let process_count = self.get_process_rows().len();
                    if process_count > 0 && self.selected_index < process_count - 1 {
                        self.selected_index += 1;
                    }
                }
            }
            Action::ToggleProcessTree => {
                if self.kill_dialog.is_none() && !self.search_mode {
                    self.show_tree = !self.show_tree;
                    self.clamp_selected_index();
                }
            }
            Action::StartProcessSearch => {
                if self.kill_dialog.is_none() {
                    self.search_prev_filter = self.filter_query.clone();
                    self.search_input = self.filter_query.clone();
                    self.search_mode = true;
                    self.status_message = None;
                    self.clamp_selected_index();
                }
            }
            Action::UpdateProcessSearch(c) => {
                if self.search_mode {
                    self.search_input.push(c);
                    self.selected_index = 0;
                    self.clamp_selected_index();
                }
            }
            Action::BackspaceProcessSearch => {
                if self.search_mode {
                    self.search_input.pop();
                    self.selected_index = 0;
                    self.clamp_selected_index();
                }
            }
            Action::ConfirmProcessSearch => {
                if self.search_mode {
                    self.filter_query = self.search_input.trim().to_string();
                    self.search_mode = false;
                    self.search_input.clear();
                    self.search_prev_filter.clear();
                    self.selected_index = 0;
                    self.status_message = None;
                    self.clamp_selected_index();
                }
            }
            Action::CancelProcessSearch => {
                if self.search_mode {
                    self.filter_query = self.search_prev_filter.clone();
                    self.search_mode = false;
                    self.search_input.clear();
                    self.search_prev_filter.clear();
                    self.selected_index = 0;
                    self.status_message = Some("Search canceled".to_string());
                    self.clamp_selected_index();
                }
            }
            Action::RequestProcessKill => {
                if !self.search_mode {
                    if let Some(row) = self.selected_row() {
                        self.kill_dialog = Some(KillDialog {
                            pid: row.pid,
                            name: row.name,
                            yes_selected: true,
                        });
                        self.status_message = None;
                    } else {
                        self.status_message = Some("No process selected".to_string());
                    }
                }
            }
            Action::ToggleProcessKillChoice => {
                if let Some(dialog) = self.kill_dialog.as_mut() {
                    dialog.yes_selected = !dialog.yes_selected;
                }
            }
            Action::ConfirmProcessKill => {
                if let Some(dialog) = self.kill_dialog.take() {
                    if dialog.yes_selected {
                        self.kill_process(dialog.pid, &dialog.name);
                    } else {
                        self.status_message = Some("Termination canceled".to_string());
                    }
                }
            }
            Action::CancelProcessKill => {
                if self.kill_dialog.take().is_some() {
                    self.status_message = Some("Termination canceled".to_string());
                }
            }
            _ => {}
        }

        self.clamp_selected_index();
        Ok(None)
    }

    fn render(&mut self, f: &mut Frame) {
        self.render_in_area(f, f.area());
    }
}
