use sysinfo::{Disk, Disks};

/// A single physical disk summarised for display.
pub(crate) struct DiskRow {
    pub(crate) name: String,
    pub(crate) label: String,
    pub(crate) used: u64,
    pub(crate) total: u64,
}

/// Collapses disks by device, keeping the shortest mount label, sorted by size.
pub(crate) fn disk_rows(disks: &Disks) -> Vec<DiskRow> {
    let mut rows: Vec<DiskRow> = Vec::new();

    for disk in disks.list() {
        if !is_real_disk(disk) {
            continue;
        }

        let name = disk.name().to_string_lossy().into_owned();
        let mount = disk.mount_point().to_string_lossy().into_owned();
        let used = disk.total_space().saturating_sub(disk.available_space());

        if let Some(row) = rows.iter_mut().find(|row| row.name == name) {
            if mount.len() < row.label.len() {
                row.label = mount;
            }
            continue;
        }

        rows.push(DiskRow {
            name,
            label: mount,
            used,
            total: disk.total_space(),
        });
    }

    rows.sort_by_key(|row| std::cmp::Reverse(row.total));
    rows
}

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
}
