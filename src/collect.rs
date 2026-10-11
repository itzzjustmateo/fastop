//! Data collection layer.
//!
//! [`Collector`] owns every OS handle (sysinfo, the GPU backend and the
//! battery manager) and produces the rows and snapshots the UI renders.
//! Keeping collection out of the UI makes the panels pure and lets new
//! panels reuse this data without touching the application state.

use std::collections::HashMap;
use std::time::Instant;

use sysinfo::{
    Components, Disks, MINIMUM_CPU_UPDATE_INTERVAL, Networks, ProcessesToUpdate, System, Uid, Users,
};

use crate::battery::BatteryMonitor;
use crate::disks::{self, DiskRow};
use crate::gpu::Gpu;
use crate::network::{NetworkRow, is_network_interface};
use crate::processes::{ProcessRow, SortBy, compare_processes};
use crate::sensors::cpu_temperature;
use crate::temperature::TemperatureHistory;

/// Number of temperature samples kept for the CPU/GPU sparkline graphs.
const TEMPERATURE_HISTORY: usize = 120;

/// Which optional collectors should be activated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CollectOptions {
    /// Whether to detect and sample a GPU.
    pub(crate) gpu: bool,
    /// Whether to detect and sample the batteries.
    pub(crate) battery: bool,
}

impl Default for CollectOptions {
    fn default() -> Self {
        Self {
            gpu: true,
            battery: true,
        }
    }
}

/// A point-in-time view of the system memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MemorySnapshot {
    /// Used physical memory in bytes.
    pub(crate) used: u64,
    /// Total physical memory in bytes.
    pub(crate) total: u64,
    /// Used swap in bytes.
    pub(crate) used_swap: u64,
    /// Total swap in bytes.
    pub(crate) total_swap: u64,
}

/// Owns every data source and refreshes it on demand.
pub(crate) struct Collector {
    system: System,
    components: Components,
    disks: Disks,
    networks: Networks,
    users: Users,
    user_names: HashMap<Uid, String>,
    gpu: Option<Gpu>,
    battery: BatteryMonitor,
    network_tick: Instant,
    cpu_name: String,
    host_name: String,
    os_name: String,
    cpu_temp: Option<f32>,
    cpu_temp_history: TemperatureHistory,
    gpu_temp_history: TemperatureHistory,
    process_rows: Vec<ProcessRow>,
    network_rows: Vec<NetworkRow>,
    disk_rows: Vec<DiskRow>,
}

impl Collector {
    /// Builds the collector, samples every source once and starts the GPU
    /// backend when enabled.
    pub(crate) fn new(options: CollectOptions) -> Self {
        let mut system = System::new_all();
        std::thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL);
        system.refresh_cpu_usage();

        let cpu_name = system
            .cpus()
            .first()
            .map_or_else(|| "Unknown CPU".to_string(), |cpu| cpu.brand().to_string());
        let host_name = System::host_name().unwrap_or_else(|| "unknown".into());
        let os_name = System::long_os_version().unwrap_or_else(|| "unknown OS".into());

        let mut collector = Self {
            system,
            components: Components::new_with_refreshed_list(),
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
            users: Users::new_with_refreshed_list(),
            user_names: HashMap::new(),
            gpu: if options.gpu { Gpu::detect() } else { None },
            battery: if options.battery {
                BatteryMonitor::new()
            } else {
                BatteryMonitor::disabled()
            },
            network_tick: Instant::now(),
            cpu_name,
            host_name,
            os_name,
            cpu_temp: None,
            cpu_temp_history: TemperatureHistory::new(TEMPERATURE_HISTORY),
            gpu_temp_history: TemperatureHistory::new(TEMPERATURE_HISTORY),
            process_rows: Vec::new(),
            network_rows: Vec::new(),
            disk_rows: Vec::new(),
        };

        collector.refresh(SortBy::Cpu);
        collector
    }

    /// Re-samples every enabled source and rebuilds the display rows.
    pub(crate) fn refresh(&mut self, sort: SortBy) {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.system.refresh_processes(ProcessesToUpdate::All, true);
        self.components.refresh(false);
        self.disks.refresh(true);
        self.networks.refresh(true);
        self.users.refresh();

        if let Some(gpu) = &mut self.gpu {
            gpu.refresh();
        }

        self.cpu_temp = cpu_temperature(&self.components);
        if let Some(temp) = self.cpu_temp {
            self.cpu_temp_history.push(temp);
        }
        if let Some(temp) = self.gpu.as_ref().and_then(|gpu| gpu.temperature) {
            self.gpu_temp_history.push(temp);
        }
        self.battery.refresh();

        self.rebuild_process_rows(sort);
        self.rebuild_network_rows();
        self.disk_rows = disks::disk_rows(&self.disks);
    }

    /// Re-orders the cached process rows for a new sort key without
    /// re-sampling the system.
    pub(crate) fn resort(&mut self, sort: SortBy) {
        self.process_rows
            .sort_by(|a, b| compare_processes(sort, a, b));
    }

    /// Global CPU usage across all cores, 0–100.
    pub(crate) fn global_cpu_usage(&self) -> f32 {
        self.system.global_cpu_usage()
    }

    /// Per-core CPU handles used by the CPU panel.
    pub(crate) fn cpus(&self) -> &[sysinfo::Cpu] {
        self.system.cpus()
    }

    /// Manufacturer/model name of the first CPU.
    pub(crate) fn cpu_name(&self) -> &str {
        &self.cpu_name
    }

    /// Latest memory snapshot.
    pub(crate) fn memory(&self) -> MemorySnapshot {
        MemorySnapshot {
            used: self.system.used_memory(),
            total: self.system.total_memory(),
            used_swap: self.system.used_swap(),
            total_swap: self.system.total_swap(),
        }
    }

    /// The detected GPU backend, when enabled and present.
    pub(crate) fn gpu(&self) -> Option<&Gpu> {
        self.gpu.as_ref()
    }

    /// Most representative CPU temperature, when a sensor exists.
    pub(crate) fn cpu_temp(&self) -> Option<f32> {
        self.cpu_temp
    }

    /// Rolling CPU temperature history for the sparkline.
    pub(crate) fn cpu_temp_history(&self) -> &TemperatureHistory {
        &self.cpu_temp_history
    }

    /// Rolling GPU temperature history for the sparkline.
    pub(crate) fn gpu_temp_history(&self) -> &TemperatureHistory {
        &self.gpu_temp_history
    }

    /// Sorted process rows for the table.
    pub(crate) fn process_rows(&self) -> &[ProcessRow] {
        &self.process_rows
    }

    /// Network interface rows sorted by throughput.
    pub(crate) fn network_rows(&self) -> &[NetworkRow] {
        &self.network_rows
    }

    /// Physical disk rows sorted by size.
    pub(crate) fn disk_rows(&self) -> &[DiskRow] {
        &self.disk_rows
    }

    /// Battery monitoring state.
    pub(crate) fn battery(&self) -> &BatteryMonitor {
        &self.battery
    }

    /// Machine host name.
    pub(crate) fn host_name(&self) -> &str {
        &self.host_name
    }

    /// Long operating system version string.
    pub(crate) fn os_name(&self) -> &str {
        &self.os_name
    }

    fn rebuild_process_rows(&mut self, sort: SortBy) {
        self.refresh_user_names();

        let user_names = &self.user_names;
        let mut rows: Vec<ProcessRow> = self
            .system
            .processes()
            .iter()
            .map(|(pid, process)| {
                let user = process
                    .user_id()
                    .and_then(|uid| user_names.get(uid))
                    .map_or_else(|| "?".to_string(), |name| name.clone());

                ProcessRow {
                    pid: pid.as_u32(),
                    user,
                    name: process.name().to_string_lossy().into_owned(),
                    cpu: process.cpu_usage(),
                    memory: process.memory(),
                }
            })
            .collect();

        rows.sort_by(|a, b| compare_processes(sort, a, b));
        self.process_rows = rows;
    }

    fn refresh_user_names(&mut self) {
        self.user_names = self
            .users
            .list()
            .iter()
            .map(|user| (user.id().clone(), user.name().to_string()))
            .collect();
    }

    fn rebuild_network_rows(&mut self) {
        let now = Instant::now();
        let elapsed = now
            .duration_since(self.network_tick)
            .as_secs_f64()
            .max(0.05);
        self.network_tick = now;

        let mut rows: Vec<NetworkRow> = self
            .networks
            .list()
            .iter()
            .filter(|(name, _)| is_network_interface(name))
            .map(|(name, data)| NetworkRow {
                name: name.clone(),
                received: data.received() as f64 / elapsed,
                transmitted: data.transmitted() as f64 / elapsed,
            })
            .collect();

        rows.sort_by(|a, b| {
            (b.received + b.transmitted)
                .total_cmp(&(a.received + a.transmitted))
                .then_with(|| a.name.cmp(&b.name))
        });

        self.network_rows = rows;
    }
}
