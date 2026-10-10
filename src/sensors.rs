use sysinfo::Components;

/// Picks the most CPU-representative temperature from the available sensors.
pub(crate) fn cpu_temperature(components: &Components) -> Option<f32> {
    let mut best: Option<(u8, f32)> = None;

    for component in components.iter() {
        let Some(temperature) = component.temperature() else {
            continue;
        };
        let Some(score) = cpu_temperature_score(component.label()) else {
            continue;
        };

        if best.is_none_or(|(current, _)| score > current) {
            best = Some((score, temperature));
        }
    }

    best.map(|(_, temperature)| temperature)
}

/// Scores how likely a sensor label belongs to the CPU (higher is better).
fn cpu_temperature_score(label: &str) -> Option<u8> {
    let label = label.to_ascii_lowercase();

    if ["package", "tctl", "tdie"]
        .into_iter()
        .any(|needle| label.contains(needle))
    {
        Some(3)
    } else if ["k10temp", "coretemp", "zenpower", "cpu"]
        .into_iter()
        .any(|needle| label.contains(needle))
    {
        Some(2)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_cpu_temperature_labels() {
        assert_eq!(cpu_temperature_score("k10temp Tctl"), Some(3));
        assert_eq!(cpu_temperature_score("Package id 0"), Some(3));
        assert_eq!(cpu_temperature_score("coretemp Core 0"), Some(2));
        assert_eq!(cpu_temperature_score("zenpower"), Some(2));
        assert_eq!(cpu_temperature_score("amdgpu edge"), None);
        assert_eq!(cpu_temperature_score("nvme Composite"), None);
    }
}
