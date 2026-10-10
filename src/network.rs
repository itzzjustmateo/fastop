/// A network interface summarised with current transfer rates.
pub(crate) struct NetworkRow {
    pub(crate) name: String,
    /// Bytes received per second.
    pub(crate) received: f64,
    /// Bytes transmitted per second.
    pub(crate) transmitted: f64,
}

/// Whether an interface is worth showing (i.e. not loopback or virtual).
pub(crate) fn is_network_interface(name: &str) -> bool {
    let name = name.to_ascii_lowercase();

    if name == "lo" {
        return false;
    }

    const VIRTUAL_PREFIXES: [&str; 8] = [
        "veth", "docker", "br-", "virbr", "vmnet", "dummy", "ifb", "awdl",
    ];

    !VIRTUAL_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_virtual_network_interfaces() {
        assert!(is_network_interface("wlan0"));
        assert!(is_network_interface("eth0"));
        assert!(is_network_interface("en0"));
        assert!(!is_network_interface("lo"));
        assert!(!is_network_interface("veth1a2b"));
        assert!(!is_network_interface("docker0"));
        assert!(!is_network_interface("br-9f2c"));
        assert!(!is_network_interface("virbr0"));
    }
}
