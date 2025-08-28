//! NUMA-aware buffer allocation for multi-socket systems.

#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::path::Path;

use crate::buffer::allocation::allocate_aligned_buffer;

/// Allocates a buffer with NUMA affinity on Linux systems.
#[cfg(target_os = "linux")]
pub fn allocate_numa_buffer(size: usize, numa_node: Option<usize>) -> Box<[u8]> {
    match numa_node {
        Some(node) => {
            // Try NUMA-aware allocation if available
            if is_numa_available() {
                match try_numa_allocation(size, node) {
                    Ok(buffer) => buffer,
                    Err(_) => {
                        // Fall back to regular allocation on failure
                        allocate_aligned_buffer(size)
                    }
                }
            } else {
                allocate_aligned_buffer(size)
            }
        }
        None => allocate_aligned_buffer(size),
    }
}

/// Try NUMA-aware allocation by setting CPU affinity.
#[cfg(target_os = "linux")]
fn try_numa_allocation(size: usize, node: usize) -> Result<Box<[u8]>, std::io::Error> {
    // Simplified NUMA allocation approach:
    // 1. Bind current thread to CPUs on the specified NUMA node
    // 2. Allocate memory (Linux will prefer local memory)
    // 3. Return to original affinity

    // Save current CPU affinity
    let original_affinity = get_current_affinity()?;

    // Set affinity to NUMA node CPUs
    if let Err(e) = set_numa_affinity(node) {
        return Err(e);
    }

    // Allocate buffer (kernel will prefer local memory)
    let result = allocate_aligned_buffer(size);

    // Restore original affinity (best effort)
    let _ = set_cpu_affinity(&original_affinity);

    Ok(result)
}

/// Get current CPU affinity mask.
#[cfg(target_os = "linux")]
fn get_current_affinity() -> Result<libc::cpu_set_t, std::io::Error> {
    use std::mem;

    unsafe {
        let mut cpu_set: libc::cpu_set_t = mem::zeroed();
        if libc::sched_getaffinity(0, mem::size_of::<libc::cpu_set_t>(), &mut cpu_set) == 0 {
            Ok(cpu_set)
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
}

/// Set CPU affinity to CPUs on the specified NUMA node.
#[cfg(target_os = "linux")]
fn set_numa_affinity(node: usize) -> Result<(), std::io::Error> {
    use std::mem;

    // Get CPUs for this NUMA node
    let cpus = get_numa_node_cpus(node)?;

    if cpus.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("No CPUs found for NUMA node {}", node),
        ));
    }

    unsafe {
        let mut cpu_set: libc::cpu_set_t = mem::zeroed();
        libc::CPU_ZERO(&mut cpu_set);

        // Set CPU bits for this NUMA node
        for cpu in cpus {
            if cpu < 1024 {
                // CPU_SETSIZE limit
                libc::CPU_SET(cpu, &mut cpu_set);
            }
        }

        if libc::sched_setaffinity(0, mem::size_of::<libc::cpu_set_t>(), &cpu_set) == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
}

/// Set specific CPU affinity.
#[cfg(target_os = "linux")]
fn set_cpu_affinity(cpu_set: &libc::cpu_set_t) -> Result<(), std::io::Error> {
    use std::mem;

    unsafe {
        if libc::sched_setaffinity(0, mem::size_of::<libc::cpu_set_t>(), cpu_set) == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
}

/// Get list of CPUs for a NUMA node by reading sysfs.
#[cfg(target_os = "linux")]
fn get_numa_node_cpus(node: usize) -> Result<Vec<usize>, std::io::Error> {
    let path = format!("/sys/devices/system/node/node{}/cpulist", node);

    if !Path::new(&path).exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("NUMA node {} not found", node),
        ));
    }

    let cpulist = fs::read_to_string(&path)?;
    parse_cpu_list(&cpulist.trim())
}

/// Parse CPU list format (e.g., "0-7,16-23" -> [0,1,2,3,4,5,6,7,16,17,18,19,20,21,22,23]).
#[allow(dead_code)]
fn parse_cpu_list(cpulist: &str) -> Result<Vec<usize>, std::io::Error> {
    let mut cpus = Vec::new();

    for part in cpulist.split(',') {
        if part.contains('-') {
            // Range format: "0-7"
            let range: Vec<&str> = part.split('-').collect();
            if range.len() == 2 {
                let start: usize = range[0].parse().map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid CPU range")
                })?;
                let end: usize = range[1].parse().map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid CPU range")
                })?;

                for cpu in start..=end {
                    cpus.push(cpu);
                }
            }
        } else {
            // Single CPU: "16"
            let cpu: usize = part.parse().map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid CPU number")
            })?;
            cpus.push(cpu);
        }
    }

    Ok(cpus)
}

/// Check if NUMA is available on the system.
#[cfg(target_os = "linux")]
pub fn is_numa_available() -> bool {
    Path::new("/sys/devices/system/node").exists()
        && fs::read_dir("/sys/devices/system/node")
            .map(|entries| entries.count() > 1) // More than just "node0"
            .unwrap_or(false)
}

/// Allocates a buffer with NUMA preference (stub for non-Linux platforms).
#[cfg(not(target_os = "linux"))]
pub fn allocate_numa_buffer(size: usize, _numa_node: Option<usize>) -> Box<[u8]> {
    allocate_aligned_buffer(size)
}

/// Gets the NUMA node for the current CPU (Linux only).
#[cfg(target_os = "linux")]
pub fn current_numa_node() -> Option<usize> {
    // Method 1: Try to get from /proc/self/stat
    if let Ok(stat) = fs::read_to_string("/proc/self/stat") {
        // CPU is field 39 (0-indexed: field 38)
        let fields: Vec<&str> = stat.split_whitespace().collect();
        if let Some(cpu_str) = fields.get(38) {
            if let Ok(cpu) = cpu_str.parse::<usize>() {
                // Try to determine NUMA node for this CPU
                if let Ok(node) = get_cpu_numa_node(cpu) {
                    return Some(node);
                }
            }
        }
    }

    // Method 2: Use getcpu system call if available
    #[cfg(target_arch = "x86_64")]
    {
        unsafe {
            let mut cpu: libc::c_uint = 0;
            let mut node: libc::c_uint = 0;

            // Try getcpu syscall
            if libc::syscall(
                libc::SYS_getcpu,
                &mut cpu,
                &mut node,
                std::ptr::null_mut::<libc::c_void>(),
            ) == 0
            {
                return Some(node as usize);
            }
        }
    }

    None
}

/// Get NUMA node for a specific CPU.
#[cfg(target_os = "linux")]
fn get_cpu_numa_node(cpu: usize) -> Result<usize, std::io::Error> {
    // Read from /sys/devices/system/cpu/cpuX/node
    let path = format!("/sys/devices/system/cpu/cpu{}/node", cpu);

    if Path::new(&path).exists() {
        let node_str = fs::read_to_string(&path)?;
        node_str
            .trim()
            .parse()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid NUMA node"))
    } else {
        // Fallback: assume CPU 0-7 -> node 0, 8-15 -> node 1, etc.
        Ok(cpu / 8)
    }
}

/// Get the number of NUMA nodes on the system.
#[cfg(target_os = "linux")]
pub fn numa_node_count() -> usize {
    if let Ok(entries) = fs::read_dir("/sys/devices/system/node") {
        entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.file_name().to_string_lossy().starts_with("node")
                    && entry
                        .file_name()
                        .to_string_lossy()
                        .chars()
                        .skip(4)
                        .all(|c| c.is_ascii_digit())
            })
            .count()
    } else {
        1 // Default to single node
    }
}

/// Get the number of NUMA nodes (non-Linux stub).
#[cfg(not(target_os = "linux"))]
pub fn numa_node_count() -> usize {
    1
}

/// Check if NUMA is available on the system (non-Linux stub).
#[cfg(not(target_os = "linux"))]
pub fn is_numa_available() -> bool {
    false
}

/// Gets the NUMA node for the current CPU (non-Linux stub).
#[cfg(not(target_os = "linux"))]
pub fn current_numa_node() -> Option<usize> {
    None
}
