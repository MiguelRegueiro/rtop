use crate::data::snapshot::{
    BatteryInfo, DiskInfo, NetworkInfo, ProcessInfo, SystemSnapshot, TemperatureInfo,
};
use nvml_wrapper::Nvml;
use std::collections::{HashMap, VecDeque};
use sysinfo::{
    ComponentExt, CpuExt, DiskExt, NetworkExt, NetworksExt, PidExt, ProcessExt, System, SystemExt,
};

pub struct DataCollector {
    system: System,
    previous_network_values: HashMap<String, (u64, u64)>, // Store previous (received, transmitted) for rate calculation
    last_update_time: std::time::Instant,
    cpu_history: Vec<VecDeque<f32>>,
    memory_history: VecDeque<(u64, u64)>,
    swap_history: VecDeque<(u64, u64)>,
    network_history: VecDeque<(u64, u64)>,
    disk_usage_history: Vec<VecDeque<(u64, u64)>>,
    nvml: Option<Nvml>,
    #[cfg(target_os = "linux")]
    lspci_gpu_candidates: Vec<(String, String)>, // (name, vendor)
    #[cfg(target_os = "linux")]
    intel_drm_card_path: Option<String>,
    #[cfg(target_os = "linux")]
    intel_rc6_paths: Vec<String>,
    #[cfg(target_os = "linux")]
    intel_gt_cur_freq_paths: Vec<String>,
    #[cfg(target_os = "linux")]
    intel_gt_max_freq_paths: Vec<String>,
    #[cfg(target_os = "linux")]
    intel_gt_min_freq_paths: Vec<String>,
    #[cfg(target_os = "linux")]
    intel_gpu_busy_percent_path: Option<String>,
    #[cfg(target_os = "linux")]
    intel_temp_input_path: Option<String>,
    #[cfg(target_os = "linux")]
    intel_debugfs_mem_path: Option<String>,
    #[cfg(target_os = "linux")]
    previous_intel_rc6_by_path: HashMap<String, (u64, std::time::Instant)>,
    #[cfg(target_os = "linux")]
    intel_gpu_rapl_energy_path: Option<String>,
    #[cfg(target_os = "linux")]
    previous_intel_gpu_energy: Option<f64>,
    #[cfg(target_os = "linux")]
    previous_intel_gpu_time: Option<std::time::Instant>,
    #[cfg(target_os = "linux")]
    previous_intel_gpu_usage: Option<f32>,
    #[cfg(target_os = "linux")]
    previous_rapl_energy: Option<f64>,
    #[cfg(target_os = "linux")]
    previous_rapl_time: Option<std::time::Instant>,
    process_cpu_ema: HashMap<u32, f32>,
}

impl DataCollector {
    pub fn new() -> Self {
        // Initialize System with only essential components to reduce startup time
        let mut system = System::new();
        system.refresh_system(); // Only get system-level info initially

        Self {
            system,
            previous_network_values: HashMap::new(),
            last_update_time: std::time::Instant::now(),
            cpu_history: Vec::new(),
            memory_history: VecDeque::with_capacity(Self::HISTORY_LEN),
            swap_history: VecDeque::with_capacity(Self::HISTORY_LEN),
            network_history: VecDeque::with_capacity(Self::HISTORY_LEN),
            disk_usage_history: Vec::new(),
            nvml: Self::initialize_nvml(),
            #[cfg(target_os = "linux")]
            lspci_gpu_candidates: Self::detect_lspci_gpus(),
            #[cfg(target_os = "linux")]
            intel_drm_card_path: Self::detect_intel_drm_card_path(),
            #[cfg(target_os = "linux")]
            intel_rc6_paths: Vec::new(),
            #[cfg(target_os = "linux")]
            intel_gt_cur_freq_paths: Vec::new(),
            #[cfg(target_os = "linux")]
            intel_gt_max_freq_paths: Vec::new(),
            #[cfg(target_os = "linux")]
            intel_gt_min_freq_paths: Vec::new(),
            #[cfg(target_os = "linux")]
            intel_gpu_busy_percent_path: None,
            #[cfg(target_os = "linux")]
            intel_temp_input_path: None,
            #[cfg(target_os = "linux")]
            intel_debugfs_mem_path: None,
            #[cfg(target_os = "linux")]
            previous_intel_rc6_by_path: HashMap::new(),
            #[cfg(target_os = "linux")]
            intel_gpu_rapl_energy_path: None,
            #[cfg(target_os = "linux")]
            previous_intel_gpu_energy: None,
            #[cfg(target_os = "linux")]
            previous_intel_gpu_time: None,
            #[cfg(target_os = "linux")]
            previous_intel_gpu_usage: None,
            #[cfg(target_os = "linux")]
            previous_rapl_energy: None,
            #[cfg(target_os = "linux")]
            previous_rapl_time: None,
            process_cpu_ema: HashMap::new(),
        }
    }

    const HISTORY_LEN: usize = 120;

    fn initialize_nvml() -> Option<Nvml> {
        // Initialize NVML but catch any errors that might occur during initialization
        match Nvml::init() {
            Ok(nvml) => {
                // Test if we can actually access any device to make sure NVML is really working
                if let Ok(count) = nvml.device_count() {
                    if count > 0 {
                        Some(nvml)
                    } else {
                        // No NVIDIA GPUs found, don't initialize NVML
                        None
                    }
                } else {
                    // Error getting device count, don't initialize NVML
                    None
                }
            }
            Err(_) => {
                // NVML initialization failed, return None
                None
            }
        }
    }
    pub fn collect(&mut self) -> SystemSnapshot {
        // More granular refreshes to improve performance
        self.system.refresh_cpu();
        self.system.refresh_memory();
        self.system.refresh_networks_list(); // Refresh network list separately
        self.system.refresh_disks_list();
        self.system.refresh_disks();
        self.system.refresh_components(); // Refresh components separately
                                          // Only refresh processes if needed (configurable)
        self.system.refresh_processes(); // Refresh processes separately

        let elapsed = self.last_update_time.elapsed().as_secs_f64();
        self.last_update_time = std::time::Instant::now();

        let cpu_count = self.system.cpus().len();
        if self.cpu_history.len() != cpu_count {
            self.cpu_history = (0..cpu_count)
                .map(|_| VecDeque::with_capacity(Self::HISTORY_LEN))
                .collect();
        }

        for (i, cpu) in self.system.cpus().iter().enumerate() {
            Self::push_history_point(&mut self.cpu_history[i], cpu.cpu_usage());
        }

        Self::push_history_point(
            &mut self.memory_history,
            (self.system.used_memory(), self.system.total_memory()),
        );
        Self::push_history_point(
            &mut self.swap_history,
            (self.system.used_swap(), self.system.total_swap()),
        );

        // Update network interfaces and calculate rates
        self.system.refresh_networks();
        // Collect network data to get count and iterate
        let network_data: Vec<_> = self
            .system
            .networks()
            .iter()
            .map(|(name, data)| {
                (
                    name.clone(),
                    data.total_received(),
                    data.total_transmitted(),
                )
            })
            .collect();

        let mut network_interfaces = HashMap::with_capacity(network_data.len());
        let mut networks = Vec::with_capacity(network_data.len());

        for (interface_name, current_received, current_transmitted) in network_data {
            let current_data = (current_received, current_transmitted);
            network_interfaces.insert(interface_name.to_string(), current_data);

            // Calculate rates based on previous values
            let (received_per_sec, transmitted_per_sec) =
                if let Some(&(prev_received, prev_transmitted)) =
                    self.previous_network_values.get(&interface_name)
                {
                    let received_diff = current_received.saturating_sub(prev_received) as f64;
                    let transmitted_diff =
                        current_transmitted.saturating_sub(prev_transmitted) as f64;

                    let received_rate = if elapsed > 0.0 {
                        received_diff / elapsed
                    } else {
                        0.0
                    };
                    let transmitted_rate = if elapsed > 0.0 {
                        transmitted_diff / elapsed
                    } else {
                        0.0
                    };

                    (received_rate as u64, transmitted_rate as u64)
                } else {
                    (0, 0)
                };

            // Update previous values for next calculation
            self.previous_network_values.insert(
                interface_name.to_string(),
                (current_received, current_transmitted),
            );

            networks.push(NetworkInfo {
                name: interface_name.to_string(),
                total_received: current_received,
                total_transmitted: current_transmitted,
                received_per_sec,
                transmitted_per_sec,
            });
        }
        self.previous_network_values
            .retain(|name, _| network_interfaces.contains_key(name));

        let total_rx_rate = networks.iter().map(|n| n.received_per_sec).sum::<u64>();
        let total_tx_rate = networks.iter().map(|n| n.transmitted_per_sec).sum::<u64>();
        Self::push_history_point(&mut self.network_history, (total_rx_rate, total_tx_rate));

        // Update temperature sensors
        let temperature_sensors = self.update_temperature_sensors();

        // Update CPU frequencies with kernel-file fallbacks for systems where sysinfo reports 0.
        let cpu_frequencies = self.collect_cpu_frequencies(cpu_count);

        // sysinfo process CPU semantics differ across platforms/versions:
        // some report 0..100, others 0..(cores*100). Normalize only when needed.
        let cpu_normalization = cpu_count.max(1) as f32;
        let smoothing_alpha = (elapsed as f32 / 1.5).clamp(0.35, 1.0);
        let mut next_process_cpu_ema: HashMap<u32, f32> =
            HashMap::with_capacity(self.system.processes().len());

        // Create process info - only collect essential information to reduce memory usage
        let processes: Vec<ProcessInfo> = self
            .system
            .processes()
            .values()
            .map(|process| {
                let pid = process.pid().as_u32();
                let raw_cpu = process.cpu_usage();
                let normalized_cpu = if raw_cpu > 100.0 {
                    (raw_cpu / cpu_normalization).clamp(0.0, 100.0)
                } else {
                    raw_cpu.clamp(0.0, 100.0)
                };
                let smoothed_cpu = self
                    .process_cpu_ema
                    .get(&pid)
                    .map(|prev| prev + (normalized_cpu - prev) * smoothing_alpha)
                    .unwrap_or(normalized_cpu);
                next_process_cpu_ema.insert(pid, smoothed_cpu);

                ProcessInfo {
                    pid,
                    name: process.name().to_string(),
                    memory: process.memory(),
                    cpu_usage: smoothed_cpu,
                    disk_usage: process.disk_usage().total_written_bytes,
                    parent_pid: process.parent().map(|pid| pid.as_u32()),
                    cmd: {
                        let cmd = process.cmd();
                        if cmd.len() > 3 {
                            // Only keep first few command args to save memory
                            cmd.iter().take(3).map(|s| s.to_string()).collect()
                        } else {
                            cmd.iter().map(|s| s.to_string()).collect()
                        }
                    },
                    exe: {
                        // Only store exe path if it's reasonably short to save memory
                        let exe_path = process.exe().to_string_lossy();
                        if exe_path.len() < 200 {
                            // Limit path length
                            Some(exe_path.to_string())
                        } else {
                            None // Skip storing very long paths
                        }
                    },
                    root: None, // Skip root path to save memory
                    cwd: None,  // Skip current working directory to save memory
                    status: format!("{:?}", process.status()),
                }
            })
            .collect();
        self.process_cpu_ema = next_process_cpu_ema;

        // Create deduplicated disk info (avoid double-counting btrfs subvolumes/multi-mount entries).
        let disks = self.collect_disks();

        // Collect GPU info
        let mut gpus: Vec<crate::data::snapshot::GpuInfo> = Vec::with_capacity(4); // Assume max 4 GPUs to pre-allocate
        #[cfg(target_os = "linux")]
        {
            self.ensure_intel_gpu_paths();
        }
        #[cfg(target_os = "linux")]
        let (intel_usage, intel_usage_note) = self.get_intel_gpu_usage_with_note();
        #[cfg(not(target_os = "linux"))]
        let (intel_usage, intel_usage_note): (Option<f32>, Option<String>) = (None, None);
        #[cfg(target_os = "linux")]
        let (intel_temp, intel_temp_note) =
            self.get_intel_gpu_temperature_with_note(&temperature_sensors);
        #[cfg(not(target_os = "linux"))]
        let (intel_temp, intel_temp_note): (Option<f32>, Option<String>) = (None, None);
        #[cfg(target_os = "linux")]
        let (intel_mem_used, intel_mem_note) = self.get_intel_gpu_memory_usage_with_note();
        #[cfg(not(target_os = "linux"))]
        let (intel_mem_used, intel_mem_note): (Option<u64>, Option<String>) = (None, None);
        #[cfg(target_os = "linux")]
        let intel_mem_total = self.get_intel_gpu_shared_memory_total(intel_mem_used);
        #[cfg(not(target_os = "linux"))]
        let intel_mem_total: Option<u64> = None;
        #[cfg(target_os = "linux")]
        let (intel_gpu_power, intel_power_note) = self.get_intel_gpu_power_consumption_with_note();
        #[cfg(not(target_os = "linux"))]
        let (intel_gpu_power, intel_power_note): (Option<f32>, Option<String>) = (None, None);

        // Collect NVIDIA GPUs using NVML
        if let Some(nvml) = &self.nvml {
            if let Ok(device_count) = nvml.device_count() {
                for i in 0..device_count.min(4) {
                    // Limit to max 4 GPUs to prevent excessive allocation
                    if let Ok(device) = nvml.device_by_index(i) {
                        let name = device
                            .name()
                            .unwrap_or_else(|_| "Unknown NVIDIA GPU".to_string());
                        let vendor = "NVIDIA".to_string();
                        let temp = device
                            .temperature(
                                nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu,
                            )
                            .map(|t| t as f32)
                            .ok();
                        let usage = device.utilization_rates().map(|util| util.gpu as f32).ok();
                        let (mem_used, mem_total) = device
                            .memory_info()
                            .map_or((None, None), |mem| (Some(mem.used), Some(mem.total)));
                        let power_usage = device.power_usage().map(|p| p as f32 / 1000.0).ok(); // Convert from milliwatts to watts

                        gpus.push(crate::data::snapshot::GpuInfo {
                            name,
                            vendor,
                            temp,
                            usage,
                            usage_note: None,
                            memory_used: mem_used,
                            memory_total: mem_total,
                            power_usage,
                            temp_note: None,
                            power_note: None,
                            memory_note: None,
                        });
                    }
                }
            }
        }

        // Collect other GPUs using sysinfo components (Intel integrated, AMD, etc.)
        for component in self.system.components().iter().take(4) {
            // Limit to first 4 components to reduce memory
            let label_lower = component.label().to_lowercase();

            // Check if this component represents a GPU
            let is_gpu_component = label_lower.contains("gpu")
                || label_lower.contains("vga")
                || label_lower.contains("graphics")
                || label_lower.contains("display")
                || label_lower.contains("intel")
                    && (label_lower.contains("hd")
                        || label_lower.contains("iris")
                        || label_lower.contains("uhd")
                        || label_lower.contains("xe")
                        || label_lower.contains("intel"))
                || label_lower.contains("amd") && !label_lower.contains("nvidia"); // AMD GPU but not NVIDIA

            // For Intel processors, also check if the component might be related to the CPU (integrated graphics)
            let is_intel_integrated = label_lower.contains("intel")
                && (
                    label_lower.contains("core") ||  // Intel Core processors often have integrated graphics
                label_lower.contains("pentium") ||
                label_lower.contains("celeron") ||
                label_lower.contains("xeon") ||
                label_lower.contains("arc")
                    // Intel Arc integrated graphics
                )
                && (
                    label_lower.contains("graphics")
                        || label_lower.contains("gpu")
                        || component.temperature() > 0.0
                    // Valid temperature reading suggests it's a real component
                );

            let is_gpu_component = is_gpu_component || is_intel_integrated;

            // Only add if it's a GPU component and not a duplicate of an NVIDIA GPU we already detected
            if is_gpu_component {
                let name = component.label().to_string();

                // Determine vendor based on the component name
                let vendor = if label_lower.contains("nvidia") {
                    "NVIDIA".to_string()
                } else if label_lower.contains("intel") {
                    "Intel".to_string()
                } else if label_lower.contains("amd")
                    || label_lower.contains("radeon")
                    || label_lower.contains("ati")
                {
                    "AMD".to_string()
                } else {
                    "Unknown".to_string()
                };

                // Only add if it's not a duplicate NVIDIA GPU (in case NVML failed but sysinfo detected it)
                let is_duplicate_nvidia = gpus
                    .iter()
                    .any(|gpu| gpu.vendor == "NVIDIA" && name.to_lowercase().contains("nvidia"));

                if !is_duplicate_nvidia {
                    let has_nvml_nvidia = gpus
                        .iter()
                        .any(|gpu| gpu.vendor == "NVIDIA" && gpu.memory_total.is_some());
                    if vendor == "NVIDIA" && has_nvml_nvidia {
                        continue;
                    }

                    // Try to get usage data based on vendor
                    let usage = match vendor.as_str() {
                        "Intel" => intel_usage,
                        _ => None, // Other vendors don't have special usage reading methods yet
                    };

                    // Also try to get temperature for Intel GPUs from sysfs if available
                    let temp = if vendor == "Intel" {
                        intel_temp.or_else(|| {
                            let temp = component.temperature();
                            if temp > 0.0 {
                                Some(temp)
                            } else {
                                None
                            }
                        })
                    } else {
                        Some(component.temperature())
                    };
                    let power_usage = if vendor == "Intel" {
                        intel_gpu_power
                    } else {
                        None
                    };
                    let memory_used = if vendor == "Intel" {
                        intel_mem_used
                    } else {
                        None
                    };
                    let memory_total = if vendor == "Intel" {
                        intel_mem_total
                    } else {
                        None
                    };
                    let temp_note = if vendor == "Intel" && temp.is_none() {
                        intel_temp_note.clone()
                    } else {
                        None
                    };
                    let power_note = if vendor == "Intel" && power_usage.is_none() {
                        intel_power_note.clone()
                    } else {
                        None
                    };
                    let memory_note = if vendor == "Intel" && memory_used.is_none() {
                        intel_mem_note.clone()
                    } else {
                        None
                    };
                    let usage_note = if vendor == "Intel" {
                        intel_usage_note.clone()
                    } else {
                        None
                    };

                    gpus.push(crate::data::snapshot::GpuInfo {
                        name,
                        vendor,
                        temp,
                        usage, // May be from sysfs for Intel GPUs
                        usage_note,
                        memory_used,
                        memory_total, // Intel iGPU memory is shared system RAM
                        power_usage,
                        temp_note,
                        power_note,
                        memory_note,
                    });
                }
            }
        }

        // Get CPU power consumption before other operations to avoid borrow checker issues
        let cpu_power = self.get_cpu_power_consumption();

        // Linux fallback GPU detection via cached lspci data (captured once at startup)
        #[cfg(target_os = "linux")]
        {
            for (name, vendor) in &self.lspci_gpu_candidates {
                if gpus.len() >= 8 {
                    break;
                }

                // If NVML already provided NVIDIA devices, don't add lspci NVIDIA fallback entries.
                let has_nvml_nvidia = gpus
                    .iter()
                    .any(|gpu| gpu.vendor == "NVIDIA" && gpu.memory_total.is_some());
                if vendor == "NVIDIA" && has_nvml_nvidia {
                    continue;
                }

                let already_detected = gpus.iter().any(|gpu| {
                    gpu.name.eq_ignore_ascii_case(name)
                        || gpu.name.to_lowercase().contains(&name.to_lowercase())
                });
                if already_detected {
                    continue;
                }

                let usage = if vendor == "Intel" { intel_usage } else { None };
                let power_usage = if vendor == "Intel" {
                    intel_gpu_power
                } else {
                    None
                };
                let temp_note = if vendor == "Intel" && intel_temp.is_none() {
                    intel_temp_note.clone()
                } else {
                    None
                };
                let power_note = if vendor == "Intel" && power_usage.is_none() {
                    intel_power_note.clone()
                } else {
                    None
                };
                let memory_note = if vendor == "Intel" && intel_mem_used.is_none() {
                    intel_mem_note.clone()
                } else {
                    None
                };
                let usage_note = if vendor == "Intel" {
                    intel_usage_note.clone()
                } else {
                    None
                };

                gpus.push(crate::data::snapshot::GpuInfo {
                    name: name.clone(),
                    vendor: vendor.clone(),
                    temp: intel_temp,
                    usage,
                    usage_note,
                    memory_used: if vendor == "Intel" {
                        intel_mem_used
                    } else {
                        None
                    },
                    memory_total: if vendor == "Intel" {
                        intel_mem_total
                    } else {
                        None
                    },
                    power_usage,
                    temp_note,
                    power_note,
                    memory_note,
                });
            }

            // If Intel telemetry exists but no Intel GPU entry was discovered through sensors/lspci,
            // add a synthetic iGPU row so integrated metrics are still visible.
            let has_intel_entry = gpus.iter().any(|gpu| gpu.vendor == "Intel");
            if !has_intel_entry && self.intel_drm_card_path.is_some() {
                gpus.push(crate::data::snapshot::GpuInfo {
                    name: "Intel Integrated Graphics".to_string(),
                    vendor: "Intel".to_string(),
                    temp: intel_temp,
                    usage: intel_usage,
                    usage_note: intel_usage_note.clone(),
                    memory_used: intel_mem_used,
                    memory_total: intel_mem_total,
                    power_usage: intel_gpu_power,
                    temp_note: if intel_temp.is_none() {
                        intel_temp_note.clone()
                    } else {
                        None
                    },
                    power_note: if intel_gpu_power.is_none() {
                        intel_power_note.clone()
                    } else {
                        None
                    },
                    memory_note: if intel_mem_used.is_none() {
                        intel_mem_note.clone()
                    } else {
                        None
                    },
                });
            }
        }

        self.update_disk_history();

        SystemSnapshot {
            global_cpu_usage: self.system.global_cpu_info().cpu_usage(),
            used_memory: self.system.used_memory(),
            total_memory: self.system.total_memory(),
            used_swap: self.system.used_swap(),   // Added
            total_swap: self.system.total_swap(), // Added
            cpu_count,
            cached_memory: self.get_cached_memory(),
            cpu_history: self.cpu_history.clone(),
            memory_history: self.memory_history.clone(),
            swap_history: self.swap_history.clone(),
            network_interfaces,
            selected_network_interface: None,
            cpu_frequencies,
            network_history: self.network_history.clone(),
            disk_usage_history: self.disk_usage_history.clone(),
            temperature_sensors,
            battery_info: self.update_battery_info(),
            processes,
            disks,
            networks,
            gpus, // Assign collected GPUs
            hostname: self.get_hostname(),
            uptime: self.get_uptime(),
            load_avg: self.get_load_avg(),
            process_sort_by: crate::data::snapshot::ProcessSortBy::CpuUsage,
            chart_type: crate::data::snapshot::ChartType::CpuUsage,
            color_scheme: crate::data::snapshot::ColorScheme::Default,
            auto_update: true,
            cpu_power, // Assign the collected CPU power
            cpu_name: if let Some(cpu) = self.system.cpus().first() {
                cpu.brand().to_string()
            } else {
                "Unknown CPU".to_string()
            },
            update_interval: 1000, // Default value, should be configurable
            show_colors: true,     // Default value, should be configurable
            show_graphs: true,     // Default value, should be configurable
        }
    }

    fn update_disk_history(&mut self) {
        let disks = self.collect_disks();
        if self.disk_usage_history.len() > disks.len() {
            self.disk_usage_history.truncate(disks.len());
        }

        for (i, disk) in disks.iter().enumerate() {
            while self.disk_usage_history.len() <= i {
                self.disk_usage_history
                    .push(VecDeque::with_capacity(Self::HISTORY_LEN));
            }

            let available_space = disk.available_space;
            let total_space = disk.total_space;
            Self::push_history_point(
                &mut self.disk_usage_history[i],
                (available_space, total_space),
            );
        }
    }

    fn collect_disks(&self) -> Vec<DiskInfo> {
        // Key by (name, filesystem, total_space) and keep the smallest available space
        // among duplicates to avoid under-reporting usage.
        let mut by_key: HashMap<(String, String, u64), DiskInfo> = HashMap::new();

        for disk in self.system.disks() {
            let total_space = disk.total_space();
            if total_space == 0 {
                continue;
            }

            let name = disk.name().to_string_lossy().to_string();
            let fs = String::from_utf8_lossy(disk.file_system()).to_string();
            let available_space = disk.available_space();
            let key = (name.clone(), fs, total_space);

            by_key
                .entry(key)
                .and_modify(|entry| {
                    entry.available_space = entry.available_space.min(available_space);
                })
                .or_insert(DiskInfo {
                    name,
                    total_space,
                    available_space,
                });
        }

        let mut disks: Vec<DiskInfo> = by_key.into_values().collect();
        disks.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then_with(|| b.total_space.cmp(&a.total_space))
        });
        disks
    }

    fn push_history_point<T>(queue: &mut VecDeque<T>, value: T) {
        queue.push_back(value);
        while queue.len() > Self::HISTORY_LEN {
            queue.pop_front();
        }
    }

    #[cfg(target_os = "linux")]
    fn detect_lspci_gpus() -> Vec<(String, String)> {
        use std::process::Command;
        let mut candidates = Vec::new();

        let output = match Command::new("lspci").output() {
            Ok(output) => output,
            Err(_) => return candidates,
        };
        let output_str = String::from_utf8_lossy(&output.stdout);

        for line in output_str.lines().take(64) {
            let line_lower = line.to_lowercase();
            let is_gpu = line_lower.contains("vga")
                || line_lower.contains("3d controller")
                || line_lower.contains("display controller");
            if !is_gpu {
                continue;
            }

            let raw_desc = line
                .split_once(':')
                .map(|(_, rest)| rest.trim())
                .unwrap_or(line)
                .trim();
            let raw_desc = raw_desc
                .split_once(' ')
                .map(|(_, rest)| rest.trim())
                .unwrap_or(raw_desc);
            let name = clean_gpu_description(raw_desc);
            let lower_name = name.to_lowercase();
            let vendor = if lower_name.contains("nvidia") {
                "NVIDIA"
            } else if lower_name.contains("intel") || lower_name.contains("arc") {
                "Intel"
            } else if lower_name.contains("amd")
                || lower_name.contains("ati")
                || lower_name.contains("radeon")
            {
                "AMD"
            } else {
                "Unknown"
            };

            let already_present = candidates
                .iter()
                .any(|(candidate, _)| candidate.eq_ignore_ascii_case(&name));
            if !already_present {
                candidates.push((name, vendor.to_string()));
            }
        }

        candidates
    }

    fn collect_cpu_frequencies(&self, cpu_count: usize) -> Vec<u64> {
        let mut cpu_frequencies: Vec<u64> = self
            .system
            .cpus()
            .iter()
            .map(|cpu| cpu.frequency())
            .collect();
        if cpu_frequencies.len() < cpu_count {
            cpu_frequencies.resize(cpu_count, 0);
        }

        #[cfg(target_os = "linux")]
        {
            let proc_fallback = Self::read_cpu_frequencies_from_proc_cpuinfo(cpu_count);
            for idx in 0..cpu_count {
                if cpu_frequencies[idx] == 0 {
                    if let Some(freq_mhz) = Self::read_cpu_frequency_from_sysfs(idx) {
                        cpu_frequencies[idx] = freq_mhz;
                        continue;
                    }
                    if let Some(freq_mhz) = proc_fallback.get(idx).and_then(|v| *v) {
                        cpu_frequencies[idx] = freq_mhz;
                    }
                }
            }
        }

        cpu_frequencies
    }

    #[cfg(target_os = "linux")]
    fn read_cpu_frequency_from_sysfs(cpu_idx: usize) -> Option<u64> {
        use std::fs;

        let candidates = [
            format!(
                "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_cur_freq",
                cpu_idx
            ),
            format!(
                "/sys/devices/system/cpu/cpu{}/cpufreq/cpuinfo_cur_freq",
                cpu_idx
            ),
        ];

        for path in candidates {
            let Ok(raw) = fs::read_to_string(path) else {
                continue;
            };
            let Ok(value) = raw.trim().parse::<u64>() else {
                continue;
            };
            if value == 0 {
                continue;
            }

            // cpufreq exports in kHz on Linux. If already in MHz, keep it.
            return Some(if value >= 100_000 {
                value / 1000
            } else {
                value
            });
        }

        None
    }

    #[cfg(target_os = "linux")]
    fn read_cpu_frequencies_from_proc_cpuinfo(cpu_count: usize) -> Vec<Option<u64>> {
        use std::fs;

        let mut out = vec![None; cpu_count];
        let Ok(content) = fs::read_to_string("/proc/cpuinfo") else {
            return out;
        };

        let mut current_idx: Option<usize> = None;
        for line in content.lines() {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim();

                if key == "processor" {
                    current_idx = value.parse::<usize>().ok();
                    continue;
                }

                if key == "cpu MHz" {
                    if let (Some(idx), Ok(mhz)) = (current_idx, value.parse::<f32>()) {
                        if idx < out.len() {
                            out[idx] = Some(mhz.round().max(0.0) as u64);
                        }
                    }
                }
            }
        }

        out
    }

    fn get_cached_memory(&self) -> u64 {
        // On Linux, we can get cached memory from /proc/meminfo
        #[cfg(target_os = "linux")]
        {
            use std::fs;
            if let Ok(content) = fs::read_to_string("/proc/meminfo") {
                for line in content.lines() {
                    if line.starts_with("Cached:") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            if let Ok(cached_kb) = parts[1].parse::<u64>() {
                                return cached_kb * 1024; // Convert KB to bytes
                            }
                        }
                    }
                }
            }
        }
        // For other systems, return 0 as fallback
        0
    }

    fn update_temperature_sensors(&self) -> Vec<TemperatureInfo> {
        let mut temperature_sensors = Vec::new();

        // Add all current temperature sensors
        for component in self.system.components() {
            temperature_sensors.push(TemperatureInfo {
                label: component.label().to_string(),
                temperature: component.temperature(),
            });
        }

        #[cfg(target_os = "linux")]
        {
            let has_cpu_sensor = temperature_sensors.iter().any(|sensor| {
                let label = sensor.label.to_ascii_lowercase();
                label.contains("cpu")
                    || label.contains("package")
                    || label.contains("x86_pkg_temp")
                    || label.contains("tdie")
                    || label.contains("tctl")
                    || label.contains("tcpu")
            });

            if !has_cpu_sensor {
                if let Some(temp) = Self::read_linux_thermal_zone_temp(&[
                    "x86_pkg_temp",
                    "tdie",
                    "tctl",
                    "tcpu",
                    "cpu-thermal",
                    "cpu",
                ]) {
                    temperature_sensors.push(TemperatureInfo {
                        label: "x86_pkg_temp (thermal)".to_string(),
                        temperature: temp,
                    });
                }
            }
        }

        temperature_sensors
    }

    fn update_battery_info(&self) -> Option<BatteryInfo> {
        // Get battery information based on the platform
        #[cfg(target_os = "linux")]
        {
            // Try to read battery information from /sys/class/power_supply/
            use std::fs;
            use std::path::Path;

            // Look for battery directories in power supply
            if let Ok(entries) = fs::read_dir("/sys/class/power_supply/") {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        if dir_name.starts_with("BAT") {
                            // This looks like a battery
                            let capacity_path = path.join("capacity");
                            let status_path = path.join("status");

                            let mut level = None;
                            let mut status = None;

                            if Path::exists(&capacity_path) {
                                if let Ok(capacity_str) = fs::read_to_string(&capacity_path) {
                                    if let Ok(capacity) = capacity_str.trim().parse::<f32>() {
                                        level = Some(capacity);
                                    }
                                }
                            }

                            if Path::exists(&status_path) {
                                if let Ok(status_str) = fs::read_to_string(&status_path) {
                                    status = Some(status_str.trim().to_string());
                                }
                            }

                            return Some(BatteryInfo { level, status });
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            // For macOS, we could use system commands like 'pmset -g batt'
            use std::process::Command;

            if let Ok(output) = Command::new("pmset").arg("-g").arg("batt").output() {
                let output_str = String::from_utf8_lossy(&output.stdout);

                // Parse the output to extract battery percentage and status
                if let Some(line) = output_str.lines().find(|l| l.contains("InternalBattery")) {
                    let mut level = None;
                    let mut status = None;

                    // Extract percentage
                    if let Some(percent_start) = line.find('(') {
                        if let Some(percent_end) = line.find('%') {
                            if let Ok(l) = line[percent_start + 1..percent_end].parse::<f32>() {
                                level = Some(l);
                            }
                        }
                    }

                    // Extract status (Charging, Discharging, AC attached, etc.)
                    if let Some(status_match) = line.find("Battery") {
                        let remaining_str = &line[status_match..];
                        if let Some(status_end) = remaining_str.find(';') {
                            let s = remaining_str[..status_end]
                                .replace(";", "")
                                .trim()
                                .to_string();
                            status = Some(s);
                        }
                    }

                    return Some(BatteryInfo { level, status });
                }
            }
        }

        #[cfg(windows)]
        {
            // For Windows, we could use WMI or PowerShell
            use std::process::Command;

            let mut level = None;
            let mut status = None;

            if let Ok(output) = Command::new("powershell")
                .args(&[
                    "-Command",
                    "(Get-WmiObject -Class Win32_Battery).EstimatedChargeRemaining",
                ])
                .output()
            {
                let output_str = String::from_utf8_lossy(&output.stdout);
                if let Ok(l) = output_str.trim().parse::<f32>() {
                    level = Some(l);
                }
            }

            // Get battery status
            if let Ok(output) = Command::new("powershell")
                .args(&[
                    "-Command",
                    "(Get-WmiObject -Class Win32_Battery).BatteryStatus",
                ])
                .output()
            {
                let output_str = String::from_utf8_lossy(&output.stdout);
                // Map numeric battery status to readable string
                let s = match output_str.trim() {
                    "1" => "Discharging".to_string(),
                    "2" => "AC attached".to_string(),
                    "3" => "Fully charged".to_string(),
                    "4" => "Low".to_string(),
                    "5" => "Critical".to_string(),
                    "6" => "Charging".to_string(),
                    _ => "Unknown".to_string(),
                };
                status = Some(s);
            }

            return Some(BatteryInfo { level, status });
        }

        // For other platforms or if no battery is found
        #[cfg(not(target_os = "macos"))]
        {
            // On Linux, we can check DMI information to determine if it's a laptop
            use std::fs;
            if let Ok(chassis_type) = fs::read_to_string("/sys/class/dmi/id/chassis_type") {
                let chassis_type_num = chassis_type.trim().parse::<u32>().unwrap_or(0);
                // 3 = Desktop, 8 = Portable/Laptop, 9 = Laptop, 10 = Notebook
                if chassis_type_num != 3
                    && chassis_type_num != 8
                    && chassis_type_num != 9
                    && chassis_type_num != 10
                {
                    // Not a laptop, so no battery
                    return Some(BatteryInfo {
                        level: Some(0.0),
                        status: Some("N/A".to_string()),
                    });
                }
            }
        }

        None
    }

    fn get_hostname(&self) -> String {
        hostname::get()
            .unwrap_or_else(|_| std::ffi::OsString::from("Unknown"))
            .to_string_lossy()
            .into_owned()
    }

    fn get_uptime(&self) -> String {
        // Get uptime in seconds (Linux-specific approach)
        #[cfg(target_os = "linux")]
        {
            std::fs::read_to_string("/proc/uptime")
                .ok()
                .and_then(|contents| contents.split_whitespace().next()?.parse::<f64>().ok())
                .map(|uptime| {
                    let uptime = uptime as u64;
                    let days = uptime / (24 * 3600);
                    let hours = (uptime % (24 * 3600)) / 3600;
                    let mins = (uptime % 3600) / 60;
                    format!("{}d {}h {}m", days, hours, mins)
                })
                .unwrap_or_else(|| "N/A".to_string())
        }
        #[cfg(not(target_os = "linux"))]
        {
            // For non-Linux systems, return N/A or implement alternative
            "N/A".to_string()
        }
    }

    fn get_load_avg(&self) -> String {
        // On Linux, we can get load averages
        #[cfg(target_os = "linux")]
        {
            std::fs::read_to_string("/proc/loadavg")
                .unwrap_or_else(|_| "N/A".to_string())
                .trim()
                .to_string()
        }
        #[cfg(not(target_os = "linux"))]
        {
            "N/A".to_string()
        }
    }

    #[cfg(target_os = "linux")]
    fn ensure_intel_gpu_paths(&mut self) {
        if self.intel_drm_card_path.is_none() {
            self.intel_drm_card_path = Self::detect_intel_drm_card_path();
        }
        let Some(card_path) = &self.intel_drm_card_path else {
            return;
        };
        let card_path = std::path::Path::new(card_path);

        if self.intel_rc6_paths.is_empty() {
            self.intel_rc6_paths = Self::collect_intel_gt_paths(
                card_path,
                &[("power", "rc6_residency_ms"), ("gt", "rc6_residency_ms")],
            );
            // On multi-GT Intel parts, /power/rc6_residency_ms can duplicate gt0.
            // Prefer per-GT counters when available to avoid double-weighting.
            if self.intel_rc6_paths.iter().any(|p| p.contains("/gt/")) {
                self.intel_rc6_paths.retain(|p| p.contains("/gt/"));
            }
        }
        if self.intel_gt_cur_freq_paths.is_empty() {
            self.intel_gt_cur_freq_paths = Self::collect_intel_gt_paths(
                card_path,
                &[("card", "gt_cur_freq_mhz"), ("gt", "rps_cur_freq_mhz")],
            );
        }
        if self.intel_gt_max_freq_paths.is_empty() {
            self.intel_gt_max_freq_paths = Self::collect_intel_gt_paths(
                card_path,
                &[("card", "gt_max_freq_mhz"), ("gt", "rps_max_freq_mhz")],
            );
        }
        if self.intel_gt_min_freq_paths.is_empty() {
            self.intel_gt_min_freq_paths = Self::collect_intel_gt_paths(
                card_path,
                &[("card", "gt_min_freq_mhz"), ("gt", "rps_min_freq_mhz")],
            );
        }
        if self.intel_gpu_busy_percent_path.is_none() {
            self.intel_gpu_busy_percent_path = Self::first_existing_path(&[
                card_path.join("device/gpu_busy_percent"),
                card_path.join("gpu_busy_percent"),
                card_path.join("gt/gt0/busy_percent"),
            ]);
        }
        if self.intel_temp_input_path.is_none() {
            self.intel_temp_input_path = Self::detect_drm_hwmon_temp_path(card_path);
        }
        if self.intel_debugfs_mem_path.is_none() {
            self.intel_debugfs_mem_path = Self::detect_i915_debugfs_mem_path(card_path);
        }
    }

    #[cfg(target_os = "linux")]
    fn collect_intel_gt_paths(card_path: &std::path::Path, modes: &[(&str, &str)]) -> Vec<String> {
        use std::fs;

        let mut paths = Vec::new();
        for (scope, file_name) in modes {
            match *scope {
                "card" => {
                    let p = card_path.join(file_name);
                    if p.exists() {
                        paths.push(p.to_string_lossy().to_string());
                    }
                }
                "power" => {
                    let p = card_path.join("power").join(file_name);
                    if p.exists() {
                        paths.push(p.to_string_lossy().to_string());
                    }
                }
                "gt" => {
                    let gt_dir = card_path.join("gt");
                    if let Ok(entries) = fs::read_dir(gt_dir) {
                        for entry in entries.flatten() {
                            let name = entry.file_name();
                            let name = name.to_string_lossy();
                            if !name.starts_with("gt") {
                                continue;
                            }
                            let p = entry.path().join(file_name);
                            if p.exists() {
                                paths.push(p.to_string_lossy().to_string());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        paths.sort();
        paths.dedup();
        paths
    }

    #[cfg(target_os = "linux")]
    fn get_intel_gpu_usage_with_note(&mut self) -> (Option<f32>, Option<String>) {
        use std::fs;

        if let Some(path) = &self.intel_gpu_busy_percent_path {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(raw_usage) = content.trim().parse::<f32>() {
                    if raw_usage.is_finite() {
                        let usage = raw_usage.clamp(0.0, 100.0);
                        let smooth = self.smooth_intel_gpu_usage(usage);
                        return (Some(smooth), Some("Busy".to_string()));
                    }
                }
            }
        }

        let freq_by_gt = self.read_intel_gt_freq_usage_by_gt();
        let freq_estimate = Self::median_usage(freq_by_gt.values().copied().collect());

        let (rc6_by_gt, rc6_ready) = self.read_multi_gt_rc6_busy_by_gt();
        if !rc6_by_gt.is_empty() {
            let mut filtered_any = false;
            let mut rc6_samples: Vec<f32> = Vec::with_capacity(rc6_by_gt.len());
            for (gt, busy) in &rc6_by_gt {
                if !busy.is_finite() {
                    continue;
                }
                // Meteor Lake multi-GT can expose one GT with near-constant high "busy"
                // even while another GT reports realistic idle residency. If RC6 and
                // frequency strongly disagree for the same GT, prefer the non-contradictory GTs.
                let contradictory = freq_by_gt
                    .get(gt)
                    .map(|freq| *busy > 80.0 && *freq < 60.0)
                    .unwrap_or(false);
                if contradictory {
                    filtered_any = true;
                    continue;
                }
                rc6_samples.push(*busy);
            }

            if rc6_samples.is_empty() {
                rc6_samples = rc6_by_gt.values().copied().collect();
            }

            if let Some(mut usage) = Self::median_usage(rc6_samples.clone()) {
                let mut source = if filtered_any { "RC6f" } else { "RC6" }.to_string();

                if rc6_samples.len() >= 2 {
                    let mut s = rc6_samples.clone();
                    s.sort_by(|a, b| a.total_cmp(b));
                    let spread = s[s.len() - 1] - s[0];
                    if spread > 60.0 {
                        usage = s[0];
                        source = "RC6d".to_string();
                    }
                }

                // If RC6 and frequency estimates diverge heavily, blend toward frequency
                // to avoid sticky false-high RC6 values on some Intel iGPUs.
                if let Some(freq) = freq_estimate {
                    if !filtered_any && (usage - freq).abs() > 45.0 {
                        usage = (usage * 0.4 + freq * 0.6).clamp(0.0, 100.0);
                        source = "Hybrid".to_string();
                    }
                }

                let smooth = self.smooth_intel_gpu_usage(usage);
                return (Some(smooth), Some(source));
            }
        }

        if !rc6_ready && !self.intel_rc6_paths.is_empty() {
            self.previous_intel_gpu_usage = None;
            return (None, Some("Warmup".to_string()));
        }

        // Fallback: frequency-based approximation.
        match freq_estimate {
            Some(freq_usage) => {
                let smooth = self.smooth_intel_gpu_usage(freq_usage.clamp(0.0, 100.0));
                (Some(smooth), Some("Freq".to_string()))
            }
            _ => {
                self.previous_intel_gpu_usage = None;
                (None, Some("No data".to_string()))
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn read_multi_gt_rc6_busy_by_gt(&mut self) -> (HashMap<String, f32>, bool) {
        use std::fs;

        let mut by_gt_sum: HashMap<String, (f32, u32)> = HashMap::new();
        let mut had_samples = false;
        let now = std::time::Instant::now();

        for path in self.intel_rc6_paths.clone() {
            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(current_rc6_ms) = content.trim().parse::<u64>() else {
                continue;
            };

            if let Some((prev_rc6_ms, prev_time)) =
                self.previous_intel_rc6_by_path.get(&path).copied()
            {
                let elapsed_ms = now.duration_since(prev_time).as_millis() as u64;
                if elapsed_ms > 0 {
                    let rc6_delta = current_rc6_ms.saturating_sub(prev_rc6_ms);
                    let idle_ratio = (rc6_delta as f64 / elapsed_ms as f64).clamp(0.0, 1.0);
                    let busy = ((1.0 - idle_ratio) * 100.0).clamp(0.0, 100.0) as f32;
                    had_samples = true;
                    let gt_key = Self::intel_gt_key_from_path(&path);
                    by_gt_sum
                        .entry(gt_key)
                        .and_modify(|(sum, count)| {
                            *sum += busy;
                            *count += 1;
                        })
                        .or_insert((busy, 1));
                }
            }

            self.previous_intel_rc6_by_path
                .insert(path, (current_rc6_ms, now));
        }

        let by_gt = by_gt_sum
            .into_iter()
            .filter_map(|(gt, (sum, count))| {
                if count == 0 {
                    None
                } else {
                    Some((gt, sum / count as f32))
                }
            })
            .collect();
        (by_gt, had_samples)
    }

    #[cfg(target_os = "linux")]
    fn read_intel_gt_freq_usage_by_gt(&self) -> HashMap<String, f32> {
        let mut cur_by_gt = Self::read_numeric_paths_by_gt(&self.intel_gt_cur_freq_paths, true);
        let mut max_by_gt = Self::read_numeric_paths_by_gt(&self.intel_gt_max_freq_paths, true);
        let mut min_by_gt = Self::read_numeric_paths_by_gt(&self.intel_gt_min_freq_paths, false);

        let has_gt_specific = cur_by_gt.keys().any(|k| k.starts_with("gt"));
        if has_gt_specific {
            cur_by_gt.remove("card");
            max_by_gt.remove("card");
            min_by_gt.remove("card");
        }

        let mut usage_by_gt: HashMap<String, f32> = HashMap::new();
        for (gt, cur) in cur_by_gt {
            let Some(max) = max_by_gt.get(&gt).copied() else {
                continue;
            };
            let min = min_by_gt.get(&gt).copied().unwrap_or(0.0);
            let usage = if max > min {
                ((cur - min) / (max - min)) * 100.0
            } else if max > 0.0 {
                (cur / max) * 100.0
            } else {
                continue;
            };
            if usage.is_finite() {
                usage_by_gt.insert(gt, usage.clamp(0.0, 100.0));
            }
        }
        usage_by_gt
    }

    #[cfg(target_os = "linux")]
    fn read_numeric_paths_by_gt(paths: &[String], pick_max: bool) -> HashMap<String, f32> {
        use std::fs;

        let mut values: HashMap<String, f32> = HashMap::new();
        for path in paths {
            let Ok(content) = fs::read_to_string(path) else {
                continue;
            };
            let Ok(value) = content.trim().parse::<f32>() else {
                continue;
            };
            if !value.is_finite() {
                continue;
            }
            let key = Self::intel_gt_key_from_path(path);
            values
                .entry(key)
                .and_modify(|current| {
                    if pick_max {
                        if value > *current {
                            *current = value;
                        }
                    } else if value < *current {
                        *current = value;
                    }
                })
                .or_insert(value);
        }
        values
    }

    #[cfg(target_os = "linux")]
    fn intel_gt_key_from_path(path: &str) -> String {
        if let Some(idx) = path.find("/gt/") {
            let suffix = &path[idx + 4..];
            if let Some(key) = suffix.split('/').next() {
                if key.starts_with("gt") {
                    return key.to_string();
                }
            }
        }
        "card".to_string()
    }

    fn median_usage(mut values: Vec<f32>) -> Option<f32> {
        values.retain(|v| v.is_finite());
        if values.is_empty() {
            return None;
        }
        values.sort_by(|a, b| a.total_cmp(b));
        let mid = values.len() / 2;
        if values.len() % 2 == 0 {
            Some((values[mid - 1] + values[mid]) / 2.0)
        } else {
            Some(values[mid])
        }
    }

    #[cfg(target_os = "linux")]
    fn smooth_intel_gpu_usage(&mut self, usage: f32) -> f32 {
        let smoothed = self
            .previous_intel_gpu_usage
            .map(|prev| prev + (usage - prev) * 0.6)
            .unwrap_or(usage);
        self.previous_intel_gpu_usage = Some(smoothed);
        smoothed
    }

    #[cfg(target_os = "linux")]
    fn get_intel_gpu_temperature_with_note(
        &self,
        temperature_sensors: &[TemperatureInfo],
    ) -> (Option<f32>, Option<String>) {
        use std::io::ErrorKind;

        let mut permission_denied = false;
        if let Some(path) = self.intel_temp_input_path.as_ref() {
            match Self::read_temperature_from_path(path) {
                Ok(temp) => return (Some(temp), None),
                Err(kind) => {
                    if kind == ErrorKind::PermissionDenied {
                        permission_denied = true;
                    }
                }
            }
        }

        // Fallback: package sensor proxy when dedicated iGPU sensor is unavailable.
        if let Some(proxy) = temperature_sensors.iter().find(|sensor| {
            let label = sensor.label.to_lowercase();
            label.contains("package id") || label.contains("x86_pkg_temp")
        }) {
            return (Some(proxy.temperature), Some("Pkg proxy".to_string()));
        }

        // Fallback: read thermal zones directly when sysinfo components are sparse.
        if let Some(temp) =
            Self::read_linux_thermal_zone_temp(&["x86_pkg_temp", "tcpu", "acpitz", "cpu"])
        {
            return (Some(temp), Some("Thermal".to_string()));
        }

        if permission_denied {
            (None, Some("No perm".to_string()))
        } else {
            (None, Some("N/A".to_string()))
        }
    }

    #[cfg(target_os = "linux")]
    fn read_linux_thermal_zone_temp(candidates: &[&str]) -> Option<f32> {
        use std::fs;

        let entries = fs::read_dir("/sys/class/thermal").ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(file_name) = path.file_name() else {
                continue;
            };
            let name = file_name.to_string_lossy();
            if !name.starts_with("thermal_zone") {
                continue;
            }

            let Ok(zone_type_raw) = fs::read_to_string(path.join("type")) else {
                continue;
            };
            let zone_type = zone_type_raw.trim().to_lowercase();
            if !candidates
                .iter()
                .any(|cand| zone_type.contains(&cand.to_lowercase()))
            {
                continue;
            }

            let Ok(raw) = fs::read_to_string(path.join("temp")) else {
                continue;
            };
            let Ok(value) = raw.trim().parse::<f32>() else {
                continue;
            };
            return Some(if value > 1000.0 {
                value / 1000.0
            } else {
                value
            });
        }

        None
    }

    #[cfg(target_os = "linux")]
    fn read_temperature_from_path(path: &str) -> Result<f32, std::io::ErrorKind> {
        use std::fs;

        let raw = fs::read_to_string(path)
            .map_err(|err| err.kind())?
            .trim()
            .parse::<f32>()
            .map_err(|_| std::io::ErrorKind::InvalidData)?;
        // hwmon temp is usually millidegrees Celsius.
        Ok(if raw > 1000.0 { raw / 1000.0 } else { raw })
    }

    #[cfg(target_os = "linux")]
    fn get_intel_gpu_memory_usage_with_note(&self) -> (Option<u64>, Option<String>) {
        use std::{fs, io::ErrorKind, path::Path};

        let mut no_permission = false;
        if let Some(path) = self.intel_debugfs_mem_path.as_ref() {
            match fs::read_to_string(path) {
                Ok(content) => {
                    let parsed = Self::parse_i915_gem_objects_bytes(&content);
                    if parsed.is_some() {
                        return (parsed, None);
                    }
                    return (None, Some("No data".to_string()));
                }
                Err(err) => {
                    if err.kind() == ErrorKind::PermissionDenied {
                        no_permission = true;
                    }
                }
            }
        }

        // Fallback: GEM objects are often unavailable without debugfs/capabilities.
        // Shmem is a coarse but always-available proxy for shared iGPU allocations.
        if let Some(shmem_bytes) = Self::read_proc_meminfo_key_bytes("Shmem") {
            return (Some(shmem_bytes), Some("Shared".to_string()));
        }

        // Debugfs is the only broadly available source for i915 memory usage on many kernels.
        if fs::read_dir("/sys/kernel/debug/dri").is_err() {
            return (None, Some("dbgfs off".to_string()));
        }
        if !Path::new("/sys/kernel/debug/dri").exists() {
            return (None, Some("dbgfs off".to_string()));
        }

        if no_permission {
            return (None, Some("No perm".to_string()));
        }

        (None, Some("N/A".to_string()))
    }

    #[cfg(target_os = "linux")]
    fn get_intel_gpu_shared_memory_total(&self, used: Option<u64>) -> Option<u64> {
        let mem_available = Self::read_proc_meminfo_key_bytes("MemAvailable");
        let mem_total = Self::read_proc_meminfo_key_bytes("MemTotal");

        match (mem_available, mem_total, used) {
            (Some(available), Some(total), Some(used)) => {
                // Dynamic shared budget: what's currently available + what iGPU already holds.
                let dynamic = available.saturating_add(used);
                Some(dynamic.min(total).max(used))
            }
            (Some(available), Some(total), None) => Some(available.min(total)),
            (Some(available), None, Some(used)) => Some(available.saturating_add(used)),
            (Some(available), None, None) => Some(available),
            (None, Some(total), _) => Some(total),
            (None, None, _) => {
                let total = self.system.total_memory();
                if total > 0 {
                    Some(total)
                } else {
                    None
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn read_proc_meminfo_key_bytes(key: &str) -> Option<u64> {
        use std::fs;

        let content = fs::read_to_string("/proc/meminfo").ok()?;
        for line in content.lines() {
            if !line.starts_with(key) {
                continue;
            }
            // Format example: \"MemTotal:       32229848 kB\"
            let mut parts = line.split_whitespace();
            let _label = parts.next()?;
            let value = parts.next()?.parse::<u64>().ok()?;
            let unit = parts.next().unwrap_or("kB");
            return Some(match unit {
                "kB" | "KB" => value.saturating_mul(1024),
                _ => value,
            });
        }
        None
    }

    #[cfg(target_os = "linux")]
    fn parse_i915_gem_objects_bytes(content: &str) -> Option<u64> {
        // Common i915 debugfs format includes "... <bytes> bytes".
        // We pick the largest integer immediately preceding "bytes".
        let mut best: Option<u64> = None;
        for line in content.lines() {
            if !line.to_lowercase().contains("bytes") {
                continue;
            }
            let tokens: Vec<&str> = line.split_whitespace().collect();
            for i in 1..tokens.len() {
                if tokens[i].to_lowercase().contains("bytes") {
                    let n = tokens[i - 1].replace(',', "");
                    if let Ok(value) = n.parse::<u64>() {
                        best = Some(best.map_or(value, |prev| prev.max(value)));
                    }
                }
            }
        }
        best
    }

    #[cfg(target_os = "linux")]
    fn detect_intel_drm_card_path() -> Option<String> {
        use std::fs;
        use std::path::Path;

        let base = Path::new("/sys/class/drm");
        let Ok(entries) = fs::read_dir(base) else {
            return None;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(file_name) = path.file_name() else {
                continue;
            };
            let name = file_name.to_string_lossy();
            if !name.starts_with("card") || name.contains('-') {
                continue;
            }
            let vendor_path = path.join("device/vendor");
            let Ok(vendor) = fs::read_to_string(vendor_path) else {
                continue;
            };
            if vendor.trim().eq_ignore_ascii_case("0x8086") {
                return Some(path.to_string_lossy().to_string());
            }
        }
        None
    }

    #[cfg(target_os = "linux")]
    fn first_existing_path(paths: &[std::path::PathBuf]) -> Option<String> {
        paths
            .iter()
            .find(|p| p.exists())
            .map(|p| p.to_string_lossy().to_string())
    }

    #[cfg(target_os = "linux")]
    fn detect_drm_hwmon_temp_path(card_path: &std::path::Path) -> Option<String> {
        use std::fs;
        let hwmon_dir = card_path.join("device/hwmon");
        let entries = fs::read_dir(hwmon_dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path().join("temp1_input");
            if path.exists() {
                return Some(path.to_string_lossy().to_string());
            }
        }
        None
    }

    #[cfg(target_os = "linux")]
    fn detect_i915_debugfs_mem_path(card_path: &std::path::Path) -> Option<String> {
        use std::path::Path;

        let card_name = card_path.file_name()?.to_string_lossy();
        if !card_name.starts_with("card") {
            return None;
        }
        let card_idx = &card_name["card".len()..];
        let direct = Path::new("/sys/kernel/debug/dri")
            .join(card_idx)
            .join("i915_gem_objects");
        if direct.exists() {
            return Some(direct.to_string_lossy().to_string());
        }
        None
    }

    /// Gets CPU power consumption from RAPL (Running Average Power Limit) interface
    /// Note: Requires appropriate permissions - user may need to be in 'video' or 'power' group
    #[cfg(target_os = "linux")]
    fn get_cpu_power_consumption(&mut self) -> Option<f32> {
        use std::fs;
        use std::path::Path;

        // Try multiple RAPL domains to find available power data
        let rapl_paths = [
            "/sys/class/powercap/intel-rapl/intel-rapl:0/energy_uj", // Package 0
            "/sys/class/powercap/intel-rapl:0/energy_uj",            // Package 0 (alternative path)
            "/sys/class/powercap/intel-rapl/intel-rapl:0/core:0/energy_uj", // Core 0 of package 0
            "/sys/class/powercap/intel-rapl/intel-rapl:1/energy_uj", // Package 1 (for dual socket systems)
            "/sys/class/powercap/intel-rapl:1/energy_uj",            // Package 1 (alternative path)
            "/sys/class/powercap/intel-rapl/intel-rapl:0/subzone0/energy_uj", // Subzone 0
            "/sys/class/powercap/intel-rapl/intel-rapl:0/subzone1/energy_uj", // Subzone 1
        ];

        for path in &rapl_paths {
            if Path::new(path).exists() {
                if let Ok(content) = fs::read_to_string(path) {
                    if let Ok(current_energy_uj) = content.trim().parse::<f64>() {
                        let current_time = std::time::Instant::now();

                        // Calculate power if we have previous readings
                        if let (Some(prev_energy), Some(prev_time)) =
                            (self.previous_rapl_energy, self.previous_rapl_time)
                        {
                            let time_diff = (current_time - prev_time).as_secs_f64();

                            if time_diff > 0.0 && time_diff <= 10.0 {
                                // Allow up to 10 seconds between readings
                                // Power = Energy difference / Time difference
                                // Energy is in microjoules, time in seconds, so result is in microwatts
                                // Convert to watts by dividing by 1,000,000
                                let power_watts =
                                    (current_energy_uj - prev_energy) / time_diff / 1_000_000.0;

                                // Update stored values
                                self.previous_rapl_energy = Some(current_energy_uj);
                                self.previous_rapl_time = Some(current_time);

                                // Only return power if it's a reasonable value (not negative or extremely high)
                                if power_watts >= 0.0 && power_watts <= 500.0 {
                                    // Reasonable upper limit for CPU power
                                    return Some(power_watts as f32);
                                }
                            }
                        }

                        // Store the first reading
                        self.previous_rapl_energy = Some(current_energy_uj);
                        self.previous_rapl_time = Some(current_time);

                        // Continue to next path to see if we can get a better reading
                        continue;
                    }
                }
            }
        }

        // If we've gone through all paths and still don't have a power calculation,
        // return None for the first reading
        None
    }

    #[cfg(target_os = "linux")]
    #[allow(dead_code)]
    fn read_rapl_power(&mut self, rapl_path: &str) -> Option<f32> {
        use std::fs;

        if let Ok(content) = fs::read_to_string(rapl_path) {
            if let Ok(current_energy_uj) = content.trim().parse::<f64>() {
                let current_time = std::time::Instant::now();

                // Calculate power if we have previous readings
                if let (Some(prev_energy), Some(prev_time)) =
                    (self.previous_rapl_energy, self.previous_rapl_time)
                {
                    let time_diff = (current_time - prev_time).as_secs_f64();

                    if time_diff > 0.0 {
                        // Power = Energy difference / Time difference
                        // Energy is in microjoules, time in seconds, so result is in microwatts
                        // Convert to watts by dividing by 1,000,000
                        let power_watts =
                            (current_energy_uj - prev_energy) / time_diff / 1_000_000.0;

                        // Update stored values
                        self.previous_rapl_energy = Some(current_energy_uj);
                        self.previous_rapl_time = Some(current_time);

                        return Some(power_watts as f32);
                    }
                }

                // Store the first reading
                self.previous_rapl_energy = Some(current_energy_uj);
                self.previous_rapl_time = Some(current_time);

                // Return None for the first reading since we can't calculate power yet
                None
            } else {
                None
            }
        } else {
            None
        }
    }

    #[cfg(target_os = "linux")]
    fn get_intel_gpu_power_consumption_with_note(&mut self) -> (Option<f32>, Option<String>) {
        use std::fs;
        use std::path::Path;

        // Resolve and cache the first usable Intel GPU RAPL energy path.
        let energy_path = if let Some(path) = &self.intel_gpu_rapl_energy_path {
            path.clone()
        } else {
            let Some(detected) = Self::detect_intel_gpu_rapl_energy_path() else {
                return (None, Some("No RAPL".to_string()));
            };
            self.intel_gpu_rapl_energy_path = Some(detected.clone());
            detected
        };

        if !Path::new(&energy_path).exists() {
            self.intel_gpu_rapl_energy_path = None;
            self.previous_intel_gpu_energy = None;
            self.previous_intel_gpu_time = None;
            return (None, Some("No RAPL".to_string()));
        }

        let current_energy_uj = match fs::read_to_string(&energy_path) {
            Ok(content) => match content.trim().parse::<f64>() {
                Ok(value) => value,
                Err(_) => return (None, Some("Invalid".to_string())),
            },
            Err(err) => {
                if err.kind() == std::io::ErrorKind::PermissionDenied {
                    return (None, Some("No perm".to_string()));
                }
                return (None, Some("Unreadable".to_string()));
            }
        };
        let current_time = std::time::Instant::now();

        if let (Some(prev_energy), Some(prev_time)) =
            (self.previous_intel_gpu_energy, self.previous_intel_gpu_time)
        {
            let time_diff = (current_time - prev_time).as_secs_f64();
            if time_diff > 0.0 && time_diff <= 10.0 {
                // Counter can wrap; if it decreased, reset and wait for next sample.
                if current_energy_uj >= prev_energy {
                    let power_watts = (current_energy_uj - prev_energy) / time_diff / 1_000_000.0;
                    self.previous_intel_gpu_energy = Some(current_energy_uj);
                    self.previous_intel_gpu_time = Some(current_time);
                    if (0.0..=150.0).contains(&power_watts) {
                        return (Some(power_watts as f32), None);
                    }
                    return (None, Some("Outlier".to_string()));
                } else {
                    self.previous_intel_gpu_energy = Some(current_energy_uj);
                    self.previous_intel_gpu_time = Some(current_time);
                    return (None, Some("Reset".to_string()));
                }
            }
        }

        self.previous_intel_gpu_energy = Some(current_energy_uj);
        self.previous_intel_gpu_time = Some(current_time);
        (None, Some("Warmup".to_string()))
    }

    #[cfg(target_os = "linux")]
    fn detect_intel_gpu_rapl_energy_path() -> Option<String> {
        use std::fs;
        use std::path::{Path, PathBuf};

        fn candidate_from_dir(path: &Path) -> Option<PathBuf> {
            let name = fs::read_to_string(path.join("name")).ok()?;
            let normalized = name.trim().to_lowercase();
            // Core Ultra often exposes iGPU power as "uncore" instead of "gfx/gpu".
            if normalized.contains("gpu")
                || normalized.contains("gfx")
                || normalized.contains("uncore")
                || normalized.contains("psys")
            {
                let energy = path.join("energy_uj");
                if energy.exists() {
                    return Some(energy);
                }
            }
            None
        }

        let base = Path::new("/sys/class/powercap");
        if let Ok(entries) = fs::read_dir(base) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                if let Some(energy) = candidate_from_dir(&path) {
                    return Some(energy.to_string_lossy().to_string());
                }
                if let Ok(sub_entries) = fs::read_dir(&path) {
                    for sub in sub_entries.flatten() {
                        let sub_path = sub.path();
                        if !sub_path.is_dir() {
                            continue;
                        }
                        if let Some(energy) = candidate_from_dir(&sub_path) {
                            return Some(energy.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }

        // Fallback for common static layouts.
        let fallback_paths = [
            "/sys/class/powercap/intel-rapl/intel-rapl:0/gfx/energy_uj",
            "/sys/class/powercap/intel-rapl/intel-rapl:0:0/energy_uj",
            "/sys/class/powercap/intel-rapl/intel-rapl:1/gfx/energy_uj",
            "/sys/class/powercap/intel-rapl/intel-rapl:1:0/energy_uj",
        ];
        for path in fallback_paths {
            if Path::new(path).exists() {
                return Some(path.to_string());
            }
        }

        None
    }

    #[cfg(not(target_os = "linux"))]
    fn get_cpu_power_consumption(&mut self) -> Option<f32> {
        // Power consumption is not available on non-Linux systems
        None
    }
} // End of impl DataCollector

// Helper function to clean up GPU descriptions from lspci
fn clean_gpu_description(desc: &str) -> String {
    let cleaned = desc.trim();

    // Common cleanup patterns for GPU names
    let cleaned = cleaned
        .replace("VGA compatible controller: ", "")
        .replace("3D controller: ", "")
        .replace("Display controller: ", "")
        .replace("(R)", "")
        .replace("(TM)", "")
        .trim()
        .to_string();

    // Handle common Intel GPU naming patterns
    if cleaned.contains("Intel Corporation") {
        // Extract just the GPU model name after "Intel Corporation"
        if let Some(pos) = cleaned.find("Intel Corporation") {
            let after_vendor = &cleaned[pos + "Intel Corporation".len()..].trim();

            // Remove "Device XXXX" patterns and keep meaningful names
            let after_vendor = if let Some(device_pos) = after_vendor.find("Device ") {
                let before_device = &after_vendor[..device_pos].trim();
                if before_device.is_empty() {
                    // If there's nothing meaningful before "Device", try to get from after it
                    let after_device = &after_vendor[device_pos..];
                    if let Some(hex_start) = after_device.find("Device ") {
                        let hex_part = &after_device[hex_start + 7..]; // Skip "Device "
                        let hex_end = hex_part.chars().take_while(|c| c.is_alphanumeric()).count();
                        let device_code = &hex_part[..hex_end];

                        // Try to map known device codes to actual names
                        match device_code {
                            "7d55" => "Arc Graphics".to_string(), // Common for Meteor Lake
                            "a74d" | "a75d" | "a76d" => "Arc A7xxM Graphics".to_string(), // Arrow Lake
                            _ => format!("GPU (Code: {})", device_code),
                        }
                    } else {
                        "Integrated Graphics".to_string()
                    }
                } else {
                    before_device.to_string()
                }
            } else {
                after_vendor.to_string()
            };

            return format!("Intel {}", after_vendor);
        }
    }

    cleaned
}
