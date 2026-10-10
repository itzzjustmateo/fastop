//! Rolling temperature history for the CPU and GPU panels.
//!
//! The panels draw the most recent readings as a [`ratatui`] sparkline graph,
//! so samples are stored as whole degrees Celsius.

use std::collections::VecDeque;

/// A bounded window of recent temperature readings, oldest first.
pub(crate) struct TemperatureHistory {
    samples: VecDeque<u64>,
    capacity: usize,
}

impl TemperatureHistory {
    /// Creates a history that keeps at most `capacity` samples.
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Records a reading, dropping the oldest sample once the window is full.
    pub(crate) fn push(&mut self, celsius: f32) {
        if self.capacity == 0 {
            return;
        }

        // `max` maps NaN to 0.0 and negative values to 0.
        let rounded = celsius.max(0.0).round();
        self.samples.push_back(rounded as u64);

        if self.samples.len() > self.capacity {
            self.samples.pop_front();
        }
    }

    /// The most recent `width` samples, oldest first, ready for a sparkline.
    pub(crate) fn tail(&self, width: usize) -> Vec<u64> {
        let skip = self.samples.len().saturating_sub(width);
        self.samples.iter().skip(skip).copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_the_configured_capacity() {
        let mut history = TemperatureHistory::new(3);

        for value in [10.0, 20.0, 30.0, 40.0] {
            history.push(value);
        }

        assert_eq!(history.tail(10), vec![20, 30, 40]);
    }

    #[test]
    fn tail_returns_the_most_recent_samples() {
        let mut history = TemperatureHistory::new(10);

        for value in [1.0, 2.0, 3.0, 4.0, 5.0] {
            history.push(value);
        }

        assert_eq!(history.tail(2), vec![4, 5]);
        assert_eq!(history.tail(0), Vec::<u64>::new());
    }

    #[test]
    fn rounds_and_clamps_readings() {
        let mut history = TemperatureHistory::new(4);

        history.push(41.6);
        history.push(-5.0);
        history.push(f32::NAN);

        assert_eq!(history.tail(4), vec![42, 0, 0]);
    }

    #[test]
    fn zero_capacity_ignores_readings() {
        let mut history = TemperatureHistory::new(0);
        history.push(50.0);

        assert!(history.tail(10).is_empty());
    }
}
