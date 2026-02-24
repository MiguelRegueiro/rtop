/// Utility functions for the application

#[allow(dead_code)]
/// Convert bytes to human-readable format
pub fn bytes_to_human_readable(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    format!("{:.1}{}", size, UNITS[unit_idx])
}

#[allow(dead_code)]
/// Convert frequency in MHz to GHz
pub fn freq_to_ghz(mhz: u64) -> f64 {
    mhz as f64 / 1000.0
}

#[allow(dead_code)]
/// Format a duration in seconds to a human-readable string
pub fn format_duration(seconds: u64) -> String {
    let days = seconds / (24 * 3600);
    let hours = (seconds % (24 * 3600)) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if days > 0 {
        format!("{}d {}h {}m {}s", days, hours, minutes, secs)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

#[allow(dead_code)]
/// Format a percentage value to a string with 2 decimal places
pub fn format_percentage(value: f64) -> String {
    format!("{:.2}%", value)
}

#[cfg(target_os = "linux")]
#[allow(dead_code)]
/// Read Intel GPU usage from sysfs
pub fn read_intel_gpu_usage() -> Option<f32> {
    use std::fs;
    use std::path::Path;

    // First, try the direct gpu_busy_percent path which is the most accurate
    for i in 0..10 {
        // Check card0 through card9
        let path = format!("/sys/class/drm/card{}/device/gpu_busy_percent", i);
        if Path::new(&path).exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(usage) = content.trim().parse::<f32>() {
                    // Validate that the usage is in a reasonable range (0-100)
                    if usage >= 0.0 && usage <= 100.0 {
                        return Some(usage);
                    }
                }
            }
        }
    }

    // Alternative paths for newer kernels or different configurations
    let alt_paths = [
        "/sys/class/drm/card0/gpu_busy_percent",
        "/sys/class/drm/card1/gpu_busy_percent",
        "/sys/class/drm/card2/gpu_busy_percent",
        "/sys/class/drm/card3/gpu_busy_percent",
    ];

    for path in &alt_paths {
        if Path::new(path).exists() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(usage) = content.trim().parse::<f32>() {
                    // Validate that the usage is in a reasonable range (0-100)
                    if usage >= 0.0 && usage <= 100.0 {
                        return Some(usage);
                    }
                }
            }
        }
    }

    // If direct busy percent isn't available, try to get usage from other sources
    // Check for Intel GPU frequency scaling information which can indicate activity
    for i in 0..10 {
        let cur_freq_path = format!("/sys/class/drm/card{}/gt_cur_freq_mhz", i);
        let max_freq_path = format!("/sys/class/drm/card{}/gt_max_freq_mhz", i);

        if Path::new(&cur_freq_path).exists() && Path::new(&max_freq_path).exists() {
            if let (Ok(cur_content), Ok(max_content)) = (
                fs::read_to_string(&cur_freq_path),
                fs::read_to_string(&max_freq_path),
            ) {
                if let (Ok(cur_freq), Ok(max_freq)) = (
                    cur_content.trim().parse::<f32>(),
                    max_content.trim().parse::<f32>(),
                ) {
                    if max_freq > 0.0 && cur_freq >= 0.0 {
                        // Calculate usage based on current frequency vs max frequency
                        // This is a heuristic - when GPU is active, it tends to run at higher frequencies
                        let freq_usage = (cur_freq / max_freq) * 100.0;

                        // Only return if it's a reasonable value
                        if freq_usage >= 0.0 && freq_usage <= 100.0 {
                            return Some(freq_usage);
                        }
                    }
                }
            }
        }
    }

    // Another approach: Check render dump information if available
    // This is more complex but can provide better usage data
    // For now, we'll return None if we can't get usage from the simpler methods
    None
}
