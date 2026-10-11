//! Disk enumeration.
//!
//! On Linux the rows come from the kernel's physical block devices
//! (`/sys/block`), so drives that are not currently mounted — encrypted
//! partitions, USB sticks, spare disks — are listed too. Mounted
//! filesystem usage from `sysinfo` is merged in per drive. Other platforms
//! fall back to listing mounted filesystems only.

use sysinfo::{Disk, Disks};

/// Linux block devices report their size in 512-byte sectors.
#[cfg(target_os = "linux")]
const SECTOR_SIZE: u64 = 512;

/// A single physical disk summarised for display.
pub(crate) struct DiskRow {
    pub(crate) name: String,
    pub(crate) label: String,
    pub(crate) used: u64,
    pub(crate) total: u64,
}

/// A physical block device plus its capacity, as reported by the kernel.
struct PhysicalDevice {
    name: String,
    total: u64,
}

/// One mounted filesystem, deduplicated by backing device.
struct MountInfo {
    device: String,
    mount: String,
    used: u64,
    total: u64,
}

/// Builds one row per physical drive, merging mounted-filesystem usage.
pub(crate) fn disk_rows(disks: &Disks) -> Vec<DiskRow> {
    aggregate_rows(&physical_devices(), &collect_mounts(disks))
}

/// Collapses the mounted filesystems to one entry per backing device.
///
/// A single device is often mounted many times (e.g. btrfs subvolumes), so
/// only the shortest mount label is kept and its usage is counted once.
fn collect_mounts(disks: &Disks) -> Vec<MountInfo> {
    let mut mounts: Vec<MountInfo> = Vec::new();

    for disk in disks.list() {
        if !is_real_disk(disk) {
            continue;
        }

        let device = disk.name().to_string_lossy().into_owned();
        let mount = disk.mount_point().to_string_lossy().into_owned();

        if let Some(existing) = mounts.iter_mut().find(|entry| entry.device == device) {
            if mount.len() < existing.mount.len() {
                existing.mount = mount;
            }
            continue;
        }

        mounts.push(MountInfo {
            used: disk.total_space().saturating_sub(disk.available_space()),
            total: disk.total_space(),
            device,
            mount,
        });
    }

    mounts
}

/// Combines physical drives and mounted filesystems into display rows.
///
/// Every physical drive gets a row even when it holds no mounted
/// filesystem; each mount is attributed to the drive holding it. A mount
/// whose backing device is not a physical drive (LVM, LUKS mapper, network,
/// …) keeps its own row so no filesystem disappears.
fn aggregate_rows(devices: &[PhysicalDevice], mounts: &[MountInfo]) -> Vec<DiskRow> {
    let names: Vec<&str> = devices.iter().map(|device| device.name.as_str()).collect();

    let mut rows: Vec<DiskRow> = devices
        .iter()
        .map(|device| DiskRow {
            name: device.name.clone(),
            label: device.name.clone(),
            used: 0,
            total: device.total,
        })
        .collect();

    for mount in mounts {
        let device = strip_dev_prefix(&mount.device);

        match parent_device(device, &names) {
            Some(parent) => {
                if let Some(row) = rows.iter_mut().find(|row| row.name == parent) {
                    row.used = row.used.saturating_add(mount.used);
                }
            }
            None => match rows.iter_mut().find(|row| row.name == device) {
                Some(row) => row.used = row.used.saturating_add(mount.used),
                None => rows.push(DiskRow {
                    name: device.to_string(),
                    label: mount.mount.clone(),
                    used: mount.used,
                    total: mount.total,
                }),
            },
        }
    }

    rows.sort_by_key(|row| std::cmp::Reverse(row.total));
    rows
}

/// The physical block devices exposed by the kernel (whole disks only).
#[cfg(target_os = "linux")]
fn physical_devices() -> Vec<PhysicalDevice> {
    let mut devices = Vec::new();

    let Ok(entries) = std::fs::read_dir("/sys/block") else {
        return devices;
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();

        if !is_physical_block_name(&name) {
            continue;
        }

        let Ok(size) = std::fs::read_to_string(entry.path().join("size")) else {
            continue;
        };
        let Ok(sectors) = size.trim().parse::<u64>() else {
            continue;
        };

        let total = sectors.saturating_mul(SECTOR_SIZE);
        if total > 0 {
            devices.push(PhysicalDevice { name, total });
        }
    }

    devices
}

/// Non-Linux platforms have no sysfs block list, so only mounts are listed.
#[cfg(not(target_os = "linux"))]
fn physical_devices() -> Vec<PhysicalDevice> {
    Vec::new()
}

/// Strips a leading `/dev/`, leaving the kernel device name.
fn strip_dev_prefix(device: &str) -> &str {
    device.strip_prefix("/dev/").unwrap_or(device)
}

/// Finds the whole-disk device a partition name belongs to.
///
/// Exact matches and the longest whole-disk prefix win, so `sda2` maps to
/// `sda`, `nvme0n1p2` to `nvme0n1` and `mmcblk0p1` to `mmcblk0`.
fn parent_device<'a>(device: &str, whole_disks: &[&'a str]) -> Option<&'a str> {
    let mut best: Option<&str> = None;

    for candidate in whole_disks.iter().copied() {
        let Some(suffix) = device.strip_prefix(candidate) else {
            continue;
        };

        if (suffix.is_empty() || is_partition_suffix(suffix))
            && best.is_none_or(|current| candidate.len() > current.len())
        {
            best = Some(candidate);
        }
    }

    best
}

/// Whether `suffix` looks like a partition number: all digits, or a `p`
/// followed by digits (`nvme0n1p2`, `mmcblk0p1`).
fn is_partition_suffix(suffix: &str) -> bool {
    if let Some(rest) = suffix.strip_prefix('p') {
        return !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit());
    }

    !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
}

/// Whether a `/sys/block` entry names a physical drive rather than a
/// software device such as `zram`, `loop`, `dm-*` or `md*`.
fn is_physical_block_name(name: &str) -> bool {
    const PREFIXES: [&str; 6] = ["sd", "hd", "vd", "xvd", "nvme", "mmcblk"];
    PREFIXES.iter().any(|prefix| name.starts_with(prefix))
}

/// Whether a `sysinfo` mount represents real, user-visible storage.
fn is_real_disk(disk: &Disk) -> bool {
    disk.total_space() > 0 && !is_pseudo_fs(&disk.file_system().to_string_lossy())
}

fn is_pseudo_fs(file_system: &str) -> bool {
    matches!(
        file_system,
        "overlay"
            | "tmpfs"
            | "devtmpfs"
            | "squashfs"
            | "proc"
            | "sysfs"
            | "ramfs"
            | "autofs"
            | "cgroup"
            | "cgroup2"
            | "devpts"
            | "debugfs"
            | "tracefs"
            | "securityfs"
            | "configfs"
            | "fusectl"
            | "mqueue"
            | "hugetlbfs"
            | "bpf"
            | "binfmt_misc"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_pseudo_filesystems() {
        assert!(is_pseudo_fs("overlay"));
        assert!(is_pseudo_fs("tmpfs"));
        assert!(!is_pseudo_fs("btrfs"));
        assert!(!is_pseudo_fs("ext4"));
    }

    #[test]
    fn strips_dev_prefix() {
        assert_eq!(strip_dev_prefix("/dev/sda2"), "sda2");
        assert_eq!(strip_dev_prefix("tmpfs"), "tmpfs");
    }

    #[test]
    fn recognises_partition_suffixes() {
        assert!(is_partition_suffix("1"));
        assert!(is_partition_suffix("12"));
        assert!(is_partition_suffix("p2"));
        assert!(!is_partition_suffix(""));
        assert!(!is_partition_suffix("p"));
        assert!(!is_partition_suffix("a"));
    }

    #[test]
    fn maps_partitions_to_their_drive() {
        let disks = ["nvme0n1", "nvme0n10", "sda", "sdb"];

        assert_eq!(parent_device("sda2", &disks), Some("sda"));
        assert_eq!(parent_device("nvme0n1p2", &disks), Some("nvme0n1"));
        assert_eq!(parent_device("sdb", &disks), Some("sdb"));
        // The longest whole-disk prefix wins, so the tenth NVMe is not
        // mistaken for a partition of the first.
        assert_eq!(parent_device("nvme0n10", &disks), Some("nvme0n10"));
        assert_eq!(parent_device("zram0", &disks), None);
    }

    #[test]
    fn physical_names_exclude_virtual_devices() {
        assert!(is_physical_block_name("sda"));
        assert!(is_physical_block_name("nvme0n1"));
        assert!(is_physical_block_name("vda"));
        assert!(is_physical_block_name("mmcblk0"));
        assert!(!is_physical_block_name("zram0"));
        assert!(!is_physical_block_name("loop0"));
        assert!(!is_physical_block_name("dm-0"));
        assert!(!is_physical_block_name("md0"));
        assert!(!is_physical_block_name("sr0"));
    }

    #[test]
    fn aggregates_mounts_per_physical_drive() {
        let devices = [
            PhysicalDevice {
                name: "sda".to_string(),
                total: 1_000_000,
            },
            PhysicalDevice {
                name: "nvme0n1".to_string(),
                total: 500_000,
            },
        ];
        let mounts = [
            MountInfo {
                device: "/dev/sda2".to_string(),
                mount: "/".to_string(),
                used: 100,
                total: 900_000,
            },
            MountInfo {
                device: "/dev/sda1".to_string(),
                mount: "/boot".to_string(),
                used: 10,
                total: 100_000,
            },
            MountInfo {
                device: "/dev/mapper/crypt".to_string(),
                mount: "/mnt/secret".to_string(),
                used: 5,
                total: 400_000,
            },
        ];

        let rows = aggregate_rows(&devices, &mounts);

        let sda = rows.iter().find(|row| row.name == "sda").unwrap();
        assert_eq!(sda.used, 110);
        assert_eq!(sda.total, 1_000_000);

        // The unmounted NVMe is still listed, with no usage.
        let nvme = rows.iter().find(|row| row.name == "nvme0n1").unwrap();
        assert_eq!(nvme.used, 0);
        assert_eq!(nvme.total, 500_000);

        // A mapper device is not a physical drive, so it keeps its own row.
        let other = rows.iter().find(|row| row.name == "mapper/crypt").unwrap();
        assert_eq!(other.label, "/mnt/secret");
        assert_eq!(other.used, 5);
    }
}
