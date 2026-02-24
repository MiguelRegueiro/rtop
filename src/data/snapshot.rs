use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorScheme {
    Default,
    Dark,
    Light,
    Monochrome,
    Nord,
    SolarizedDark,
    SolarizedLight,
    Gruvbox,
    Rtop,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ProcessSortBy {
    CpuUsage,
    Memory,
    Pid,
    Name,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ChartType {
    CpuUsage,
    MemoryUsage,
    NetworkUsage,
    DiskUsage,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub memory: u64,
    pub cpu_usage: f32,
    pub disk_usage: u64,
    pub parent_pid: Option<u32>,
    pub cmd: Vec<String>,
    pub exe: Option<String>,
    pub root: Option<String>,
    pub cwd: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct NetworkInfo {
    pub name: String,
    pub total_received: u64,
    pub total_transmitted: u64,
    pub received_per_sec: u64,
    pub transmitted_per_sec: u64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DiskInfo {
    pub name: String,
    pub total_space: u64,
    pub available_space: u64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TemperatureInfo {
    pub label: String,
    pub temperature: f32,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BatteryInfo {
    pub level: Option<f32>,
    pub status: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct GpuInfo {
    pub name: String,
    pub vendor: String,
    pub temp: Option<f32>,  // Temperature in Celsius
    pub usage: Option<f32>, // Usage percentage 0-100
    pub usage_note: Option<String>,
    pub memory_used: Option<u64>,
    pub memory_total: Option<u64>,
    pub power_usage: Option<f32>, // Power usage in Watts
    pub temp_note: Option<String>,
    pub power_note: Option<String>,
    pub memory_note: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SystemSnapshot {
    pub global_cpu_usage: f32,
    pub used_memory: u64,
    pub total_memory: u64,
    pub used_swap: u64,
    pub total_swap: u64,
    pub cpu_count: usize,
    pub cached_memory: u64, // Cached memory
    pub cpu_history: Vec<VecDeque<f32>>,
    pub memory_history: VecDeque<(u64, u64)>, // Changed to VecDeque for efficient operations
    pub swap_history: VecDeque<(u64, u64)>,   // Changed to VecDeque for efficient operations
    pub network_interfaces: HashMap<String, (u64, u64)>,
    pub selected_network_interface: Option<String>,
    pub cpu_frequencies: Vec<u64>,
    pub network_history: VecDeque<(u64, u64)>, // Changed to VecDeque for efficient operations
    pub disk_usage_history: Vec<VecDeque<(u64, u64)>>, // Changed to VecDeque for efficient operations
    pub temperature_sensors: Vec<TemperatureInfo>,
    pub gpus: Vec<GpuInfo>,
    pub battery_info: Option<BatteryInfo>,
    pub processes: Vec<ProcessInfo>,
    pub disks: Vec<DiskInfo>,
    pub networks: Vec<NetworkInfo>,
    pub hostname: String,
    pub uptime: String,
    pub load_avg: String,
    pub process_sort_by: ProcessSortBy,
    pub chart_type: ChartType,
    pub color_scheme: ColorScheme,
    pub auto_update: bool,
    pub update_interval: u64,
    pub show_colors: bool,
    pub show_graphs: bool,
    pub cpu_power: Option<f32>, // CPU power consumption in Watts
    pub cpu_name: String,       // CPU name/model
}

impl Default for SystemSnapshot {
    fn default() -> Self {
        Self {
            global_cpu_usage: 0.0,
            used_memory: 0,
            total_memory: 0,
            used_swap: 0,
            total_swap: 0,
            cpu_count: 0,
            cached_memory: 0,
            cpu_history: vec![VecDeque::with_capacity(25)], // Reduced capacity
            memory_history: VecDeque::with_capacity(25),    // Changed to VecDeque
            swap_history: VecDeque::with_capacity(25),      // Changed to VecDeque
            network_interfaces: HashMap::new(),
            selected_network_interface: None,
            cpu_frequencies: vec![],
            network_history: VecDeque::with_capacity(25), // Changed to VecDeque
            disk_usage_history: vec![],                   // Will be sized appropriately
            temperature_sensors: vec![],
            gpus: vec![],
            battery_info: None,
            processes: vec![],
            disks: vec![],
            networks: vec![],
            hostname: String::new(),
            uptime: String::new(),
            load_avg: String::new(),
            process_sort_by: ProcessSortBy::CpuUsage,
            chart_type: ChartType::CpuUsage,
            color_scheme: ColorScheme::Default,
            auto_update: true,
            update_interval: 1000,
            show_colors: true,
            show_graphs: true,
            cpu_power: None,
            cpu_name: String::new(),
        }
    }
}
