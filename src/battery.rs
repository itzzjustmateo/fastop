//! Cross-platform battery monitoring backed by the `starship-battery` crate.
//!
//! The crate exposes an iterator that borrows the [`Manager`], so this module
//! snapshots each battery into an owned [`BatteryInfo`] that the UI can keep
//! between refreshes. Systems without a battery (desktops, CI runners) simply
//! report no batteries instead of failing.

use std::time::Duration;

use starship_battery::units::ratio::percent;
use starship_battery::units::thermodynamic_temperature::degree_celsius;
use starship_battery::units::time::second;
use starship_battery::{Manager, State};

/// A point-in-time snapshot of a single battery.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BatteryInfo {
    /// State of charge, 0–100%.
    pub(crate) percentage: f32,
    /// Current charge state.
    pub(crate) state: BatteryState,
    /// Battery temperature in degrees Celsius, when reported.
    pub(crate) temperature: Option<f32>,
    /// Remaining time until full, when charging.
    pub(crate) time_to_full: Option<Duration>,
    /// Remaining time until empty, when discharging.
    pub(crate) time_to_empty: Option<Duration>,
}

impl BatteryInfo {
    /// The remaining time that is meaningful for the current state.
    pub(crate) fn remaining(&self) -> Option<Duration> {
        self.time_to_empty.or(self.time_to_full)
    }
}

/// Battery charge state, decoupled from the backend enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatteryState {
    Charging,
    Discharging,
    Full,
    Empty,
    Paused,
    Unknown,
}

impl BatteryState {
    /// Short human-readable label for the panel.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Charging => "Charging",
            Self::Discharging => "Discharging",
            Self::Full => "Full",
            Self::Empty => "Empty",
            Self::Paused => "Paused",
            Self::Unknown => "Unknown",
        }
    }
}

impl From<State> for BatteryState {
    fn from(state: State) -> Self {
        match state {
            State::Charging => Self::Charging,
            State::Discharging => Self::Discharging,
            State::Full => Self::Full,
            State::Empty => Self::Empty,
            State::Paused => Self::Paused,
            State::Unknown => Self::Unknown,
        }
    }
}

/// Keeps a live, owned view of the system batteries.
pub(crate) struct BatteryMonitor {
    manager: Option<Manager>,
    batteries: Vec<BatteryInfo>,
}

impl BatteryMonitor {
    /// Builds a monitor. A missing battery subsystem is not an error.
    pub(crate) fn new() -> Self {
        Self {
            manager: Manager::new().ok(),
            batteries: Vec::new(),
        }
    }

    /// Refreshes the cached snapshots from the backend.
    pub(crate) fn refresh(&mut self) {
        let Some(manager) = &self.manager else {
            return;
        };

        let Ok(batteries) = manager.batteries() else {
            return;
        };

        self.batteries = batteries
            .flatten()
            .map(|battery| BatteryInfo {
                percentage: battery.state_of_charge().get::<percent>().clamp(0.0, 100.0),
                state: battery.state().into(),
                temperature: battery
                    .temperature()
                    .map(|value| value.get::<degree_celsius>()),
                time_to_full: battery
                    .time_to_full()
                    .map(|value| Duration::from_secs_f32(value.get::<second>())),
                time_to_empty: battery
                    .time_to_empty()
                    .map(|value| Duration::from_secs_f32(value.get::<second>())),
            })
            .collect();
    }

    /// The current snapshots (usually exactly one battery).
    pub(crate) fn batteries(&self) -> &[BatteryInfo] {
        &self.batteries
    }

    /// Whether at least one battery was detected.
    pub(crate) fn present(&self) -> bool {
        !self.batteries.is_empty()
    }
}

/// Formats a remaining time as a compact `1h 05m` / `45m` / `<1m` string.
pub(crate) fn format_duration(duration: Duration) -> String {
    let minutes = duration.as_secs() / 60;
    let hours = minutes / 60;

    if hours > 0 {
        format!("{hours}h {:02}m", minutes % 60)
    } else if minutes > 0 {
        format!("{minutes}m")
    } else {
        "<1m".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_backend_state() {
        assert_eq!(BatteryState::from(State::Charging), BatteryState::Charging);
        assert_eq!(
            BatteryState::from(State::Discharging),
            BatteryState::Discharging
        );
        assert_eq!(BatteryState::from(State::Full), BatteryState::Full);
        assert_eq!(BatteryState::from(State::Empty), BatteryState::Empty);
        assert_eq!(BatteryState::from(State::Paused), BatteryState::Paused);
        assert_eq!(BatteryState::from(State::Unknown), BatteryState::Unknown);
    }

    #[test]
    fn labels_states() {
        assert_eq!(BatteryState::Charging.label(), "Charging");
        assert_eq!(BatteryState::Discharging.label(), "Discharging");
        assert_eq!(BatteryState::Full.label(), "Full");
    }

    #[test]
    fn formats_remaining_duration() {
        assert_eq!(format_duration(Duration::from_secs(30)), "<1m");
        assert_eq!(format_duration(Duration::from_secs(45 * 60)), "45m");
        assert_eq!(format_duration(Duration::from_secs(3_900)), "1h 05m");
        assert_eq!(format_duration(Duration::from_secs(2 * 3600)), "2h 00m");
    }

    #[test]
    fn prefers_time_to_empty_then_full() {
        let mut info = BatteryInfo {
            percentage: 50.0,
            state: BatteryState::Discharging,
            temperature: None,
            time_to_full: Some(Duration::from_secs(600)),
            time_to_empty: Some(Duration::from_secs(1200)),
        };
        assert_eq!(info.remaining(), Some(Duration::from_secs(1200)));

        info.time_to_empty = None;
        assert_eq!(info.remaining(), Some(Duration::from_secs(600)));

        info.time_to_full = None;
        assert_eq!(info.remaining(), None);
    }
}
