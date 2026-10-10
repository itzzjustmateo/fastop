use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A detected GPU and the backend used to read its metrics.
pub(crate) struct Gpu {
    pub(crate) name: String,
    pub(crate) usage: f32,
    pub(crate) temperature: Option<f32>,
    backend: GpuBackend,
}

enum GpuBackend {
    Sysfs {
        usage_path: PathBuf,
        temperature_path: Option<PathBuf>,
    },
    NvidiaSmi(NvidiaShared),
}

#[derive(Clone, Default)]
struct NvidiaShared(Arc<Mutex<Option<NvidiaSample>>>);

#[derive(Clone, Copy)]
struct NvidiaSample {
    usage: f32,
    temperature: Option<f32>,
}

struct NvidiaQuery {
    name: String,
    usage: f32,
    temperature: Option<f32>,
}

const NVIDIA_POLL_INTERVAL: Duration = Duration::from_secs(1);

impl Gpu {
    /// Detects a GPU via sysfs first, then falling back to `nvidia-smi`.
    pub(crate) fn detect() -> Option<Self> {
        Self::detect_sysfs().or_else(Self::detect_nvidia)
    }

    fn detect_sysfs() -> Option<Self> {
        let entries = std::fs::read_dir("/sys/class/drm").ok()?;

        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();

            if !is_drm_card(&name) {
                continue;
            }

            let device = entry.path().join("device");
            if !device.is_dir() {
                continue;
            }

            let usage_path = device.join("gpu_busy_percent");
            if !usage_path.exists() {
                continue;
            }

            let mut gpu = Gpu {
                name: gpu_name_from_device(&device).unwrap_or_else(|| "Unknown GPU".to_string()),
                usage: 0.0,
                temperature: None,
                backend: GpuBackend::Sysfs {
                    usage_path,
                    temperature_path: gpu_temperature_path(&device),
                },
            };
            gpu.refresh();

            return Some(gpu);
        }

        None
    }

    fn detect_nvidia() -> Option<Self> {
        let first = query_nvidia_smi()?;
        let shared = spawn_nvidia_poller();

        Some(Gpu {
            name: first.name,
            usage: first.usage,
            temperature: first.temperature,
            backend: GpuBackend::NvidiaSmi(shared),
        })
    }

    /// Reads the latest usage and temperature from the active backend.
    pub(crate) fn refresh(&mut self) {
        match &self.backend {
            GpuBackend::Sysfs {
                usage_path,
                temperature_path,
            } => {
                if let Some(usage) = read_number(usage_path) {
                    self.usage = usage as f32;
                }

                self.temperature = temperature_path
                    .as_ref()
                    .and_then(|path| read_number(path))
                    .map(|milli| (milli / 1000.0) as f32);
            }
            GpuBackend::NvidiaSmi(shared) => {
                if let Ok(sample) = shared.0.lock()
                    && let Some(sample) = *sample
                {
                    self.usage = sample.usage;
                    self.temperature = sample.temperature;
                }
            }
        }
    }
}

fn is_drm_card(name: &str) -> bool {
    name.strip_prefix("card")
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
}

fn read_number(path: &std::path::Path) -> Option<f64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn gpu_temperature_path(device: &std::path::Path) -> Option<std::path::PathBuf> {
    for entry in std::fs::read_dir(device.join("hwmon")).ok()?.flatten() {
        let temperature = entry.path().join("temp1_input");
        if temperature.exists() {
            return Some(temperature);
        }
    }

    None
}

fn gpu_name_from_device(device: &std::path::Path) -> Option<String> {
    let uevent = std::fs::read_to_string(device.join("uevent")).ok()?;
    let pci_id = uevent
        .lines()
        .find_map(|line| line.strip_prefix("PCI_ID="))?;
    let (vendor, device) = pci_id.split_once(':')?;

    let vendor = u16::from_str_radix(vendor.trim(), 16).ok()?;
    let device = u16::from_str_radix(device.trim(), 16).ok()?;

    lookup_pci_name(vendor, device)
}

fn lookup_pci_name(vendor: u16, device: u16) -> Option<String> {
    const PCI_IDS_PATHS: [&str; 3] = [
        "/usr/share/hwdata/pci.ids",
        "/usr/share/misc/pci.ids",
        "/var/lib/pciutils/pci.ids",
    ];

    for path in PCI_IDS_PATHS {
        if let Ok(contents) = std::fs::read_to_string(path)
            && let Some(name) = parse_pci_ids(&contents, vendor, device)
        {
            return Some(name);
        }
    }

    None
}

fn parse_pci_ids(contents: &str, vendor: u16, device: u16) -> Option<String> {
    let vendor_line = format!("{vendor:04x}  ");
    let device_line = format!("\t{device:04x}  ");
    let mut in_vendor = false;

    for line in contents.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        if line.starts_with('\t') {
            if in_vendor && let Some(name) = line.strip_prefix(&device_line) {
                return Some(name.trim().to_string());
            }
        } else {
            in_vendor = line.starts_with(&vendor_line);
        }
    }

    None
}

fn spawn_nvidia_poller() -> NvidiaShared {
    let shared = NvidiaShared::default();
    let worker = shared.clone();

    std::thread::spawn(move || {
        loop {
            let Some(query) = query_nvidia_smi() else {
                return;
            };

            *worker.0.lock().unwrap() = Some(NvidiaSample {
                usage: query.usage,
                temperature: query.temperature,
            });

            std::thread::sleep(NVIDIA_POLL_INTERVAL);
        }
    });

    shared
}

fn query_nvidia_smi() -> Option<NvidiaQuery> {
    let output = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,utilization.gpu,temperature.gpu",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_nvidia_smi(stdout.lines().next()?)
}

fn parse_nvidia_smi(line: &str) -> Option<NvidiaQuery> {
    let mut fields = line.split(',').map(str::trim);

    let name = fields.next()?;
    if name.is_empty() {
        return None;
    }

    let usage = fields.next()?.parse::<f32>().ok()?;
    let temperature = fields.next().and_then(|value| value.parse::<f32>().ok());

    Some(NvidiaQuery {
        name: name.to_string(),
        usage,
        temperature,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_drm_card_names() {
        assert!(is_drm_card("card0"));
        assert!(is_drm_card("card12"));
        assert!(!is_drm_card("card"));
        assert!(!is_drm_card("card0-DP-1"));
        assert!(!is_drm_card("renderD128"));
    }

    #[test]
    fn parses_pci_ids_entry() {
        let contents = concat!(
            "# comment\n",
            "1002  Advanced Micro Devices, Inc. [AMD/ATI]\n",
            "\t1638  Cezanne [Radeon Vega Series]\n",
            "\t9999  Some Other Device\n",
            "8086  Intel Corporation\n",
            "\t1234  Intel Device\n",
        );

        assert_eq!(
            parse_pci_ids(contents, 0x1002, 0x1638).as_deref(),
            Some("Cezanne [Radeon Vega Series]")
        );
        assert_eq!(
            parse_pci_ids(contents, 0x8086, 0x1234).as_deref(),
            Some("Intel Device")
        );
        assert_eq!(parse_pci_ids(contents, 0x1002, 0x0000), None);
        assert_eq!(parse_pci_ids(contents, 0x10de, 0x1638), None);
    }

    #[test]
    fn parses_nvidia_smi_output() {
        let query = parse_nvidia_smi("NVIDIA GeForce RTX 3080, 42, 65").unwrap();
        assert_eq!(query.name, "NVIDIA GeForce RTX 3080");
        assert_eq!(query.usage, 42.0);
        assert_eq!(query.temperature, Some(65.0));

        let no_temperature = parse_nvidia_smi("NVIDIA GeForce RTX 3080, 42, [N/A]").unwrap();
        assert_eq!(no_temperature.temperature, None);

        assert!(parse_nvidia_smi("").is_none());
        assert!(parse_nvidia_smi("OnlyName").is_none());
        assert!(parse_nvidia_smi("GPU, not-a-number, 50").is_none());
    }
}
