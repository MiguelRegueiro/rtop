use crate::data::snapshot::{ColorScheme, ProcessSortBy, SystemSnapshot};
use crate::{
    action::Action,
    components::{
        cpu::CpuComponent, disk::DiskComponent, gpu::GpuComponent, memory::MemoryComponent,
        network::NetworkComponent, process::ProcessComponent, Component,
    },
    config::AppConfig,
    data::DataManager,
    theme::Theme,
    tui::Tui,
};
use crossterm::event::{Event as CrosstermEvent, KeyEvent, KeyEventKind, MouseEvent};
use std::time::Duration;
use tokio::sync::mpsc;

pub struct App {
    pub should_quit: bool,
    pub tui: Tui,
    pub cpu_component: CpuComponent,
    pub gpu_component: GpuComponent,
    pub memory_component: MemoryComponent,
    pub network_component: NetworkComponent,
    pub disk_component: DiskComponent,
    pub process_component: ProcessComponent,

    #[allow(dead_code)]
    pub theme: Theme,
    #[allow(dead_code)]
    pub data_manager: DataManager,
    pub snapshot: crate::data::snapshot::SystemSnapshot,
    #[allow(dead_code)]
    pub tick_rate: Duration,

    // Fields for interpolation and smooth animations
    interpolated_snapshot: crate::data::snapshot::SystemSnapshot,
    target_snapshot: crate::data::snapshot::SystemSnapshot,
    interpolation_factor: f32,
    last_update_time: std::time::Instant,
}

impl App {
    pub async fn new(tick_rate: Duration) -> Result<Self, Box<dyn std::error::Error>> {
        let tui = Tui::new()?;

        // Initialize data manager
        let mut data_manager = DataManager::new(1000); // 1 second update interval

        // Get initial snapshot
        let mut snapshot = data_manager.collector.collect();
        if let Some(config) = AppConfig::load() {
            snapshot.color_scheme = Theme::canonicalize_color_scheme(config.color_scheme);
        }

        // Initialize theme
        let theme = Theme::new(snapshot.color_scheme);
        snapshot.color_scheme = theme.color_scheme;

        // Initialize components - do this efficiently by reusing the snapshot
        let snapshot_clone = snapshot.clone();
        let theme_clone = theme.clone();

        let cpu_component = CpuComponent::new(snapshot_clone.clone(), theme_clone.clone());
        let gpu_component = GpuComponent::new(snapshot_clone.clone(), theme_clone.clone());
        let memory_component = MemoryComponent::new(snapshot_clone.clone(), theme_clone.clone());
        let network_component = NetworkComponent::new(snapshot_clone.clone(), theme_clone.clone());
        let disk_component = DiskComponent::new(snapshot_clone.clone(), theme_clone.clone());
        let process_component = ProcessComponent::new(snapshot_clone, theme_clone);

        Ok(Self {
            should_quit: false,
            tui,
            cpu_component,
            gpu_component,
            memory_component,
            network_component,
            disk_component,
            process_component,

            theme,
            data_manager,
            snapshot: snapshot.clone(),
            tick_rate,

            // Initialize interpolation fields
            interpolated_snapshot: snapshot.clone(),
            target_snapshot: snapshot.clone(),
            interpolation_factor: 1.0,
            last_update_time: std::time::Instant::now(),
        })
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Create channels for communication
        let (snapshot_tx, mut snapshot_rx) = mpsc::unbounded_channel::<SystemSnapshot>();

        // Start data collection in the background
        let mut data_manager = DataManager::new(self.snapshot.update_interval);
        tokio::spawn(async move {
            data_manager.start_polling(snapshot_tx).await;
        });

        // Set up high-frequency polling for smooth animations
        let frame_duration = Duration::from_millis(16); // ~60 FPS

        // Track if we need to redraw the UI
        let mut needs_redraw = true;
        let mut last_snapshot_hash: Option<u64> = None;

        loop {
            // Receive new snapshots first
            let mut new_snapshot_received = false;
            while let Ok(mut new_snapshot) = snapshot_rx.try_recv() {
                self.apply_ui_state_to_snapshot(&mut new_snapshot);
                // Interpolate from the currently displayed values to the new target.
                self.snapshot = self.interpolated_snapshot.clone();
                self.target_snapshot = new_snapshot;
                self.interpolation_factor = 0.0;
                new_snapshot_received = true;
            }

            // Update interpolation factor based on time elapsed
            let now = std::time::Instant::now();
            let elapsed = now.duration_since(self.last_update_time).as_secs_f32();
            self.last_update_time = now;

            if self.interpolation_factor < 1.0 {
                // About 250ms smoothing window at steady 60 FPS.
                self.interpolation_factor = (self.interpolation_factor + elapsed * 4.0).min(1.0);
            }

            // Perform linear interpolation between current and target snapshots
            self.interpolate_snapshots();

            // Calculate hash of current interpolated snapshot to determine if redraw is needed
            let current_hash = self.calculate_snapshot_hash();

            // Only redraw if the snapshot has changed significantly or we received a new snapshot
            if new_snapshot_received || last_snapshot_hash.map_or(true, |last| last != current_hash)
            {
                // Draw UI - need to separate this to avoid borrowing issues
                self.draw_frame()?;
                last_snapshot_hash = Some(current_hash);
                needs_redraw = false;
            }

            // Handle events
            if crossterm::event::poll(frame_duration)? {
                match crossterm::event::read()? {
                    CrosstermEvent::Key(key) => {
                        // Ignore key release events to avoid double-handling keys like Esc.
                        if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                            if let Some(action) = self.handle_key_events(key)? {
                                self.handle_action(action)?;
                            }
                        }
                        needs_redraw = true; // Redraw after handling actions
                    }
                    CrosstermEvent::Mouse(mouse) => {
                        if let Some(action) = self.handle_mouse_events(mouse)? {
                            self.handle_action(action)?;
                        }
                        needs_redraw = true; // Redraw after handling actions
                    }
                    CrosstermEvent::Resize(width, height) => {
                        // Force redraw on resize
                        self.tui.resize(width, height)?;
                        needs_redraw = true;
                    }
                    CrosstermEvent::FocusGained
                    | CrosstermEvent::FocusLost
                    | CrosstermEvent::Paste(_) => {
                        // Ignore these events for now
                    }
                }

                // Redraw if needed after handling events
                if needs_redraw {
                    self.draw_frame()?;
                    last_snapshot_hash = Some(self.calculate_snapshot_hash());
                    needs_redraw = false;
                }
            } else {
                // If no events were polled, yield control to allow other tasks to run
                tokio::task::yield_now().await;
            }

            // Check if we should quit
            if self.should_quit {
                break;
            }
        }

        self.tui.exit()?;
        Ok(())
    }

    /// Perform linear interpolation between current and target snapshots
    fn interpolate_snapshots(&mut self) {
        // Start from target so non-interpolated fields (process list, disks, networks, histories)
        // are always fresh and never lag behind.
        let mut new_interpolated = self.target_snapshot.clone();

        new_interpolated.global_cpu_usage = self.lerp(
            self.snapshot.global_cpu_usage,
            self.target_snapshot.global_cpu_usage,
            self.interpolation_factor,
        );

        new_interpolated.used_memory = self.lerp_u64(
            self.snapshot.used_memory,
            self.target_snapshot.used_memory,
            self.interpolation_factor,
        );

        new_interpolated.used_swap = self.lerp_u64(
            self.snapshot.used_swap,
            self.target_snapshot.used_swap,
            self.interpolation_factor,
        );

        // Interpolate GPU scalar values if available.
        let gpu_count = self
            .snapshot
            .gpus
            .len()
            .min(self.target_snapshot.gpus.len())
            .min(8);
        for i in 0..gpu_count {
            let gpu_current = &self.snapshot.gpus[i];
            let gpu_target = &self.target_snapshot.gpus[i];

            if i >= new_interpolated.gpus.len() {
                continue;
            }
            let interpolated_gpu = &mut new_interpolated.gpus[i];

            if let (Some(current_usage), Some(target_usage)) = (gpu_current.usage, gpu_target.usage)
            {
                interpolated_gpu.usage =
                    Some(self.lerp(current_usage, target_usage, self.interpolation_factor));
            }

            if let (Some(current_temp), Some(target_temp)) = (gpu_current.temp, gpu_target.temp) {
                interpolated_gpu.temp =
                    Some(self.lerp(current_temp, target_temp, self.interpolation_factor));
            }

            if let (Some(current_mem_used), Some(target_mem_used)) =
                (gpu_current.memory_used, gpu_target.memory_used)
            {
                interpolated_gpu.memory_used = Some(self.lerp_u64(
                    current_mem_used,
                    target_mem_used,
                    self.interpolation_factor,
                ));
            }
        }

        self.interpolated_snapshot = new_interpolated;

        if self.interpolation_factor >= 1.0 {
            self.snapshot = self.target_snapshot.clone();
        }

        self.sync_components();
    }

    /// Linear interpolation helper function
    fn lerp(&self, start: f32, end: f32, factor: f32) -> f32 {
        start + (end - start) * factor.clamp(0.0, 1.0)
    }

    /// Linear interpolation helper function for u64 values
    fn lerp_u64(&self, start: u64, end: u64, factor: f32) -> u64 {
        let start_f = start as f64;
        let end_f = end as f64;
        let result = start_f + (end_f - start_f) * factor as f64;
        result.round() as u64
    }

    /// Calculate a hash of the current interpolated snapshot to determine if redraw is needed
    fn calculate_snapshot_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        // Hash key values that affect the display
        self.interpolated_snapshot
            .global_cpu_usage
            .to_bits()
            .hash(&mut hasher);
        self.interpolated_snapshot.used_memory.hash(&mut hasher);
        self.interpolated_snapshot.used_swap.hash(&mut hasher);

        // Hash a sample of history data (first and last values to reduce computation)
        if let Some(cpu_hist) = self.interpolated_snapshot.cpu_history.first() {
            if let Some(first_val) = cpu_hist.front() {
                first_val.to_bits().hash(&mut hasher);
            }
            if let Some(last_val) = cpu_hist.back() {
                last_val.to_bits().hash(&mut hasher);
            }
        }

        if let Some(first_mem) = self.interpolated_snapshot.memory_history.front() {
            first_mem.hash(&mut hasher);
        }
        if let Some(last_mem) = self.interpolated_snapshot.memory_history.back() {
            last_mem.hash(&mut hasher);
        }

        // Hash network data
        self.interpolated_snapshot.networks.len().hash(&mut hasher);
        for net in &self.interpolated_snapshot.networks {
            net.received_per_sec.hash(&mut hasher);
            net.transmitted_per_sec.hash(&mut hasher);
        }

        // Hash GPU data
        for gpu in &self.interpolated_snapshot.gpus {
            if let Some(usage) = gpu.usage {
                usage.to_bits().hash(&mut hasher);
            }
            if let Some(temp) = gpu.temp {
                temp.to_bits().hash(&mut hasher);
            }
        }

        // Hash process count and some key values
        self.interpolated_snapshot.processes.len().hash(&mut hasher);
        if let Some(proc) = self.interpolated_snapshot.processes.first() {
            proc.cpu_usage.to_bits().hash(&mut hasher);
            proc.memory.hash(&mut hasher);
        }

        hasher.finish()
    }

    fn render_top_status_line(
        f: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        snapshot: &SystemSnapshot,
        theme: &Theme,
    ) {
        use ratatui::{
            style::{Color, Style},
            text::{Line, Span},
            widgets::Paragraph,
        };

        let current_time = chrono::Local::now().format("%H:%M:%S").to_string();

        let s = snapshot;
        let theme_name = Self::theme_name(s.color_scheme);
        let status_line = Line::from(vec![
            Span::styled(
                " RTOP ",
                Style::default()
                    .fg(theme.get_color(Color::Black))
                    .bg(theme.get_color(Color::Cyan))
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ),
            Span::styled(
                format!("  host:{}  ", s.hostname),
                Style::default()
                    .fg(theme.get_color(Color::White))
                    .bg(theme.get_color(Color::DarkGray)),
            ),
            Span::styled(
                format!("uptime:{}  ", s.uptime),
                Style::default()
                    .fg(theme.get_color(Color::White))
                    .bg(theme.get_color(Color::DarkGray)),
            ),
            Span::styled(
                format!("load:{}  ", s.load_avg),
                Style::default()
                    .fg(theme.get_color(Color::Yellow))
                    .bg(theme.get_color(Color::DarkGray)),
            ),
            Span::styled(
                format!("time:{} ", current_time),
                Style::default()
                    .fg(theme.get_color(Color::Green))
                    .bg(theme.get_color(Color::DarkGray)),
            ),
            Span::styled(
                format!(" theme:{} ", theme_name),
                Style::default()
                    .fg(theme.get_color(Color::LightMagenta))
                    .bg(theme.get_color(Color::DarkGray))
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ),
        ]);

        let block = Paragraph::new(status_line).style(
            Style::default()
                .fg(theme.get_color(Color::White))
                .bg(theme.get_color(Color::DarkGray)),
        );
        f.render_widget(block, area);
    }

    fn render_bottom_keybinds(
        f: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        theme: &Theme,
        active_scheme: ColorScheme,
    ) {
        use ratatui::{
            style::{Color, Style},
            text::{Line, Span},
            widgets::Paragraph,
        };

        let theme_name = Self::theme_name(active_scheme);
        let keybinds_line = Line::from(vec![
            Span::styled(
                " [q] quit ",
                Style::default().fg(theme.get_color(Color::Red)),
            ),
            Span::styled(
                " [↑/↓] move ",
                Style::default().fg(theme.get_color(Color::Green)),
            ),
            Span::styled(
                " [s] sort ",
                Style::default().fg(theme.get_color(Color::Cyan)),
            ),
            Span::styled(
                " [S] search ",
                Style::default().fg(theme.get_color(Color::LightCyan)),
            ),
            Span::styled(
                " [k] kill ",
                Style::default().fg(theme.get_color(Color::LightRed)),
            ),
            Span::styled(
                " [T] tree ",
                Style::default().fg(theme.get_color(Color::Magenta)),
            ),
            Span::styled(
                " [i] interface ",
                Style::default().fg(theme.get_color(Color::LightBlue)),
            ),
            Span::styled(
                " [t] theme ",
                Style::default().fg(theme.get_color(Color::Yellow)),
            ),
            Span::styled(
                " [w] save ",
                Style::default().fg(theme.get_color(Color::LightGreen)),
            ),
            Span::styled(
                format!("  active:{} ", theme_name),
                Style::default()
                    .fg(theme.get_color(Color::LightMagenta))
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ),
        ]);

        let block = Paragraph::new(keybinds_line).style(
            Style::default()
                .fg(theme.get_color(Color::White))
                .bg(theme.get_color(Color::DarkGray)),
        );
        f.render_widget(block, area);
    }

    fn handle_key_events(
        &mut self,
        key: KeyEvent,
    ) -> Result<Option<Action>, Box<dyn std::error::Error>> {
        if self.process_component.is_kill_confirm_active() {
            return match key.code {
                crossterm::event::KeyCode::Left
                | crossterm::event::KeyCode::Right
                | crossterm::event::KeyCode::Tab
                | crossterm::event::KeyCode::BackTab => Ok(Some(Action::ToggleProcessKillChoice)),
                crossterm::event::KeyCode::Enter => Ok(Some(Action::ConfirmProcessKill)),
                crossterm::event::KeyCode::Esc => Ok(Some(Action::CancelProcessKill)),
                _ => Ok(None),
            };
        }

        if self.process_component.is_search_mode() {
            return match key.code {
                crossterm::event::KeyCode::Esc => Ok(Some(Action::CancelProcessSearch)),
                crossterm::event::KeyCode::Enter => Ok(Some(Action::ConfirmProcessSearch)),
                crossterm::event::KeyCode::Backspace => Ok(Some(Action::BackspaceProcessSearch)),
                crossterm::event::KeyCode::Char(c) => {
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL)
                        || key.modifiers.contains(crossterm::event::KeyModifiers::ALT)
                    {
                        Ok(None)
                    } else {
                        Ok(Some(Action::UpdateProcessSearch(c)))
                    }
                }
                _ => Ok(None),
            };
        }

        match key.code {
            crossterm::event::KeyCode::Char('q') | crossterm::event::KeyCode::Esc => {
                Ok(Some(Action::Quit))
            }
            crossterm::event::KeyCode::Char('c') => Ok(Some(Action::ToggleAutoUpdate)),
            crossterm::event::KeyCode::Char('+') => Ok(Some(Action::IncreaseSpeed)),
            crossterm::event::KeyCode::Char('-') => Ok(Some(Action::DecreaseSpeed)),
            crossterm::event::KeyCode::Up => Ok(Some(Action::MoveUp)),
            crossterm::event::KeyCode::Down => Ok(Some(Action::MoveDown)),
            crossterm::event::KeyCode::Enter => Ok(Some(Action::Enter)),
            crossterm::event::KeyCode::Char('b') => Ok(Some(Action::Back)),
            crossterm::event::KeyCode::Char('t') => Ok(Some(Action::SwitchTheme)),
            crossterm::event::KeyCode::Char('k') | crossterm::event::KeyCode::Char('K') => {
                Ok(Some(Action::RequestProcessKill))
            }
            crossterm::event::KeyCode::Char('S') => Ok(Some(Action::StartProcessSearch)),
            crossterm::event::KeyCode::Char('s')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::SHIFT) =>
            {
                Ok(Some(Action::StartProcessSearch))
            }
            crossterm::event::KeyCode::Char('s') => Ok(Some(Action::SwitchProcessSort)),
            crossterm::event::KeyCode::Char('n') => Ok(Some(Action::SwitchChartType)),
            crossterm::event::KeyCode::Char('T') => Ok(Some(Action::ToggleProcessTree)),
            crossterm::event::KeyCode::Char('i') => Ok(Some(Action::CycleNetworkInterface)),
            crossterm::event::KeyCode::Char('w') => Ok(Some(Action::SaveConfig)),
            _ => Ok(None),
        }
    }

    fn handle_mouse_events(
        &mut self,
        _mouse: MouseEvent,
    ) -> Result<Option<Action>, Box<dyn std::error::Error>> {
        // Handle mouse events if needed
        Ok(None)
    }

    fn draw_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use ratatui::layout::{Constraint, Direction, Layout};

        self.tui.draw(|f| {
            let size = f.area();

            // Define main layout: Top (summary), Middle (panels + processes), Bottom (keybinds)
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // Top status line
                    Constraint::Min(0),    // Main content area
                    Constraint::Length(1), // Bottom keybinds line
                ])
                .split(size);

            // Render top status line using interpolated snapshot
            Self::render_top_status_line(
                f,
                main_chunks[0],
                &self.interpolated_snapshot,
                &self.theme,
            );

            // Split main content area into Left Panels and Central Process List
            let middle_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(33), // Left side panels (CPU, GPU, Memory)
                    Constraint::Percentage(34), // Central process list
                    Constraint::Percentage(33), // Right side panels (Network, Disk)
                ])
                .split(main_chunks[1]);

            // Split Left side for CPU, GPU, Memory
            let left_panels = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(33), // CPU
                    Constraint::Percentage(33), // GPU
                    Constraint::Percentage(34), // Memory (give the remainder to memory)
                ])
                .split(middle_chunks[0]);

            // Split Right side for Network, Disk
            let right_panels = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(50), // Network
                    Constraint::Percentage(50), // Disk
                ])
                .split(middle_chunks[2]);

            // Render Left side components
            self.cpu_component.render_in_area(f, left_panels[0]);
            self.gpu_component.render_in_area(f, left_panels[1]);
            self.memory_component.render_in_area(f, left_panels[2]);

            // Render Central process list
            self.process_component.render_in_area(f, middle_chunks[1]);

            // Render Right side components
            self.network_component.render_in_area(f, right_panels[0]);
            self.disk_component.render_in_area(f, right_panels[1]);

            // Render bottom keybinds line
            Self::render_bottom_keybinds(f, main_chunks[2], &self.theme, self.theme.color_scheme);
        })?;
        Ok(())
    }

    fn handle_action(&mut self, action: Action) -> Result<(), Box<dyn std::error::Error>> {
        match action {
            Action::Quit => {
                self.should_quit = true;
            }
            Action::ToggleAutoUpdate => {
                self.interpolated_snapshot.auto_update = !self.interpolated_snapshot.auto_update;
                self.target_snapshot.auto_update = self.interpolated_snapshot.auto_update;
                self.snapshot.auto_update = self.interpolated_snapshot.auto_update;
            }
            Action::MoveUp => {
                self.process_component.update(action.clone())?;
            }
            Action::MoveDown => {
                self.process_component.update(action.clone())?;
            }
            Action::ToggleProcessTree => {
                self.process_component.update(action.clone())?;
            }
            Action::StartProcessSearch
            | Action::UpdateProcessSearch(_)
            | Action::BackspaceProcessSearch
            | Action::ConfirmProcessSearch
            | Action::CancelProcessSearch
            | Action::RequestProcessKill
            | Action::ToggleProcessKillChoice
            | Action::ConfirmProcessKill
            | Action::CancelProcessKill => {
                self.process_component.update(action.clone())?;
            }
            Action::SwitchProcessSort => {
                let next = match self.interpolated_snapshot.process_sort_by {
                    ProcessSortBy::CpuUsage => ProcessSortBy::Memory,
                    ProcessSortBy::Memory => ProcessSortBy::Pid,
                    ProcessSortBy::Pid => ProcessSortBy::Name,
                    ProcessSortBy::Name => ProcessSortBy::CpuUsage,
                };
                self.interpolated_snapshot.process_sort_by = next;
                self.target_snapshot.process_sort_by = next;
                self.snapshot.process_sort_by = next;
            }
            Action::CycleNetworkInterface => {
                let mut names: Vec<String> = self
                    .interpolated_snapshot
                    .networks
                    .iter()
                    .map(|n| n.name.clone())
                    .collect();
                names.sort();
                names.dedup();

                let next_interface = if names.is_empty() {
                    None
                } else {
                    match &self.interpolated_snapshot.selected_network_interface {
                        None => Some(names[0].clone()),
                        Some(current) => {
                            if let Some(pos) = names.iter().position(|name| name == current) {
                                if pos + 1 < names.len() {
                                    Some(names[pos + 1].clone())
                                } else {
                                    None
                                }
                            } else {
                                Some(names[0].clone())
                            }
                        }
                    }
                };

                self.interpolated_snapshot.selected_network_interface = next_interface.clone();
                self.target_snapshot.selected_network_interface = next_interface.clone();
                self.snapshot.selected_network_interface = next_interface;
            }
            Action::SwitchTheme => {
                let cycle = Theme::cycle();
                let current = Theme::canonicalize_color_scheme(self.theme.color_scheme);
                let idx = cycle
                    .iter()
                    .position(|scheme| *scheme == current)
                    .unwrap_or(0);
                let next_scheme = cycle[(idx + 1) % cycle.len()];

                self.theme = Theme::new(next_scheme);
                self.interpolated_snapshot.color_scheme = next_scheme;
                self.target_snapshot.color_scheme = next_scheme;
                self.snapshot.color_scheme = next_scheme;
                self.sync_components();
                let _ = self.save_theme_config();
            }
            Action::SaveConfig => {
                let _ = self.save_theme_config();
            }
            _ => {
                // Handle other actions
            }
        }
        self.sync_components();
        Ok(())
    }

    fn apply_ui_state_to_snapshot(&self, snapshot: &mut SystemSnapshot) {
        snapshot.process_sort_by = self.interpolated_snapshot.process_sort_by;
        snapshot.selected_network_interface = self
            .interpolated_snapshot
            .selected_network_interface
            .as_ref()
            .and_then(|selected| {
                if snapshot
                    .networks
                    .iter()
                    .any(|network| &network.name == selected)
                {
                    Some(selected.clone())
                } else {
                    None
                }
            });
        snapshot.color_scheme = self.theme.color_scheme;
        snapshot.auto_update = self.interpolated_snapshot.auto_update;
    }

    fn sync_components(&mut self) {
        let interpolated_snapshot_clone = self.interpolated_snapshot.clone();
        self.cpu_component.snapshot = interpolated_snapshot_clone.clone();
        self.gpu_component.snapshot = interpolated_snapshot_clone.clone();
        self.memory_component.snapshot = interpolated_snapshot_clone.clone();
        self.network_component.snapshot = interpolated_snapshot_clone.clone();
        self.disk_component.snapshot = interpolated_snapshot_clone.clone();
        self.process_component.snapshot = interpolated_snapshot_clone;

        self.cpu_component.theme = self.theme.clone();
        self.gpu_component.theme = self.theme.clone();
        self.memory_component.theme = self.theme.clone();
        self.network_component.theme = self.theme.clone();
        self.disk_component.theme = self.theme.clone();
        self.process_component.theme = self.theme.clone();
    }

    fn save_theme_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config = AppConfig {
            color_scheme: self.theme.color_scheme,
        };
        config.save()
    }

    fn theme_name(scheme: ColorScheme) -> &'static str {
        match Theme::canonicalize_color_scheme(scheme) {
            ColorScheme::Default => "Graphite",
            ColorScheme::Dark => "Midnight",
            ColorScheme::Nord => "Nord",
            ColorScheme::SolarizedDark => "Solarized",
            ColorScheme::Gruvbox => "Gruvbox",
            ColorScheme::Rtop => "Neon",
            _ => "Graphite",
        }
    }
}
