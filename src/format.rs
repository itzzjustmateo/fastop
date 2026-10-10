/// Bytes-per-second scales and their short unit prefixes, largest first.
const RATE_UNITS: [(f64, char); 3] = [
    (1024.0 * 1024.0 * 1024.0, 'G'),
    (1024.0 * 1024.0, 'M'),
    (1024.0, 'K'),
];

/// Percentage of `used` out of `total`, guarding against a zero total.
pub(crate) fn percent(used: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        used as f32 / total as f32 * 100.0
    }
}

pub(crate) fn to_gib(bytes: u64) -> f64 {
    bytes as f64 / 1024.0_f64.powi(3)
}

pub(crate) fn to_mib(bytes: u64) -> f64 {
    bytes as f64 / 1024.0_f64.powi(2)
}

/// Formats a byte count as either MiB or GiB, switching at 1 GiB.
pub(crate) fn format_memory(bytes: u64) -> String {
    let mib = to_mib(bytes);

    if mib >= 1024.0 {
        format!("{:.1} GiB", to_gib(bytes))
    } else {
        format!("{mib:.1} MiB")
    }
}

/// Formats a transfer rate, e.g. `5.4 MiB/s` or `512 B/s`.
pub(crate) fn format_rate(bytes_per_second: f64) -> String {
    for (scale, prefix) in RATE_UNITS {
        if bytes_per_second >= scale {
            return format!("{:.1} {prefix}iB/s", bytes_per_second / scale);
        }
    }

    format!("{bytes_per_second:.0} B/s")
}

/// Compact transfer rate without the unit suffix, e.g. `5.4M` or `0`.
pub(crate) fn format_rate_short(bytes_per_second: f64) -> String {
    for (scale, prefix) in RATE_UNITS {
        if bytes_per_second >= scale {
            return format!("{:.1}{prefix}", bytes_per_second / scale);
        }
    }

    format!("{bytes_per_second:.0}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_handles_zero_total() {
        assert_eq!(percent(0, 0), 0.0);
        assert_eq!(percent(1024, 2048), 50.0);
    }

    #[test]
    fn byte_conversions_are_exact() {
        assert_eq!(to_gib(1024_u64.pow(3)), 1.0);
        assert_eq!(to_mib(1024_u64.pow(2)), 1.0);
    }

    #[test]
    fn format_memory_switches_units() {
        assert_eq!(format_memory(512 * 1024 * 1024), "512.0 MiB");
        assert_eq!(format_memory(2 * 1024 * 1024 * 1024), "2.0 GiB");
    }

    #[test]
    fn formats_network_rates() {
        assert_eq!(format_rate(0.0), "0 B/s");
        assert_eq!(format_rate(512.0), "512 B/s");
        assert_eq!(format_rate(1024.0), "1.0 KiB/s");
        assert_eq!(format_rate(1024.0 * 1024.0), "1.0 MiB/s");
        assert_eq!(format_rate(1024.0 * 1024.0 * 1024.0), "1.0 GiB/s");
    }

    #[test]
    fn formats_short_network_rates() {
        assert_eq!(format_rate_short(0.0), "0");
        assert_eq!(format_rate_short(1024.0), "1.0K");
        assert_eq!(format_rate_short(1024.0 * 1024.0), "1.0M");
        assert_eq!(format_rate_short(1024.0 * 1024.0 * 1024.0), "1.0G");
    }
}
