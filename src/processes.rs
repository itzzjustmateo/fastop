use ratatui::layout::Alignment;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::theme::{ACCENT, MUTED};

/// Sort key for the process table.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SortBy {
    Cpu,
    Memory,
    Pid,
    Name,
}

impl SortBy {
    pub(crate) fn label(self) -> &'static str {
        match self {
            SortBy::Cpu => "CPU",
            SortBy::Memory => "MEM",
            SortBy::Pid => "PID",
            SortBy::Name => "NAME",
        }
    }

    /// Whether the default ordering for this key is descending.
    pub(crate) fn descending(self) -> bool {
        matches!(self, SortBy::Cpu | SortBy::Memory)
    }

    /// Next sort key in the cycle used by the `s` shortcut.
    pub(crate) fn next(self) -> Self {
        match self {
            SortBy::Cpu => SortBy::Memory,
            SortBy::Memory => SortBy::Pid,
            SortBy::Pid => SortBy::Name,
            SortBy::Name => SortBy::Cpu,
        }
    }
}

/// A single process row rendered in the table.
pub(crate) struct ProcessRow {
    pub(crate) pid: u32,
    pub(crate) user: String,
    pub(crate) name: String,
    pub(crate) cpu: f32,
    pub(crate) memory: u64,
}

/// A column of the process table.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ProcessColumn {
    Pid,
    User,
    Name,
    Cpu,
    Memory,
}

/// Columns shown for the available width, dropping optional ones as it shrinks.
pub(crate) fn process_columns(width: u16) -> Vec<ProcessColumn> {
    use ProcessColumn::*;

    if width >= 56 {
        vec![Pid, User, Name, Cpu, Memory]
    } else if width >= 42 {
        vec![Pid, Name, Cpu, Memory]
    } else if width >= 28 {
        vec![Pid, Name, Cpu]
    } else {
        vec![Name, Cpu]
    }
}

/// Builds a sortable column header, adding a direction arrow when active.
pub(crate) fn header_cell(
    label: &str,
    sort: SortBy,
    column: SortBy,
    right_aligned: bool,
) -> Line<'static> {
    let mut spans = vec![Span::styled(
        label.to_string(),
        Style::default().fg(MUTED).bold(),
    )];

    if sort == column {
        let arrow = if sort.descending() { "▼" } else { "▲" };
        spans.push(Span::styled(
            format!(" {arrow}"),
            Style::default().fg(ACCENT).bold(),
        ));
    }

    let line = Line::from(spans);

    if right_aligned {
        line.alignment(Alignment::Right)
    } else {
        line
    }
}

/// Orders two process rows for the given sort key.
pub(crate) fn compare_processes(
    sort: SortBy,
    a: &ProcessRow,
    b: &ProcessRow,
) -> std::cmp::Ordering {
    match sort {
        SortBy::Cpu => b
            .cpu
            .total_cmp(&a.cpu)
            .then_with(|| b.memory.cmp(&a.memory)),
        SortBy::Memory => b
            .memory
            .cmp(&a.memory)
            .then_with(|| b.cpu.total_cmp(&a.cpu)),
        SortBy::Pid => a.pid.cmp(&b.pid),
        SortBy::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_columns_shrink_on_narrow_terminals() {
        assert_eq!(
            process_columns(120),
            vec![
                ProcessColumn::Pid,
                ProcessColumn::User,
                ProcessColumn::Name,
                ProcessColumn::Cpu,
                ProcessColumn::Memory
            ]
        );
        assert_eq!(
            process_columns(50),
            vec![
                ProcessColumn::Pid,
                ProcessColumn::Name,
                ProcessColumn::Cpu,
                ProcessColumn::Memory
            ]
        );
        assert_eq!(
            process_columns(30),
            vec![ProcessColumn::Pid, ProcessColumn::Name, ProcessColumn::Cpu]
        );
        assert_eq!(
            process_columns(20),
            vec![ProcessColumn::Name, ProcessColumn::Cpu]
        );
    }

    #[test]
    fn compares_processes_for_every_sort() {
        let a = ProcessRow {
            pid: 2,
            user: "beta".into(),
            name: "beta".into(),
            cpu: 10.0,
            memory: 500,
        };
        let b = ProcessRow {
            pid: 1,
            user: "alpha".into(),
            name: "alpha".into(),
            cpu: 20.0,
            memory: 100,
        };

        assert_eq!(
            compare_processes(SortBy::Cpu, &a, &b),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            compare_processes(SortBy::Memory, &a, &b),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            compare_processes(SortBy::Pid, &a, &b),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            compare_processes(SortBy::Name, &a, &b),
            std::cmp::Ordering::Greater
        );
    }
}
