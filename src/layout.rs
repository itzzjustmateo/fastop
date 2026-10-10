use ratatui::layout::Rect;

use crate::cli::LayoutMode;

/// Height of the full panel grid (two summary rows plus a network row).
const GRID_HEIGHT: u16 = 22;
/// Height of the single compact row of panels.
const COMPACT_HEIGHT: u16 = 9;

/// Which summary panel to draw.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PanelKind {
    Cpu,
    Memory,
    Gpu,
    Disks,
    Network,
}

/// Height reserved for the header (0, 1 or 3 lines).
pub(crate) fn header_height(area: Rect) -> u16 {
    if area.height < 4 {
        0
    } else if area.width >= 80 && area.height >= 28 {
        3
    } else {
        1
    }
}

/// Chooses between the full grid and the compact row for the given preference.
///
/// Returns whether the grid is used and how many rows the panel band needs.
pub(crate) fn panels_layout(area: Rect, mode: LayoutMode) -> (bool, u16) {
    // `Grid` accepts a tighter height so users can opt into it earlier.
    let min_grid_height = if mode == LayoutMode::Grid { 28 } else { 36 };
    let grid = mode != LayoutMode::Compact && area.width >= 80 && area.height >= min_grid_height;

    if grid {
        (true, GRID_HEIGHT)
    } else if area.width >= 44 && area.height >= 20 {
        (false, COMPACT_HEIGHT)
    } else {
        (false, 0)
    }
}

/// Panels shown in the compact row, dropping the least critical as width shrinks.
pub(crate) fn compact_panels(width: u16) -> Vec<PanelKind> {
    use PanelKind::*;

    if width >= 108 {
        vec![Cpu, Memory, Gpu, Disks, Network]
    } else if width >= 88 {
        vec![Cpu, Memory, Gpu, Network]
    } else if width >= 66 {
        vec![Cpu, Memory, Network]
    } else if width >= 44 {
        vec![Cpu, Memory]
    } else {
        vec![Cpu]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_panels_adapt_to_width() {
        assert_eq!(compact_panels(140).len(), 5);
        assert_eq!(compact_panels(120).len(), 5);
        assert_eq!(compact_panels(100).len(), 4);
        assert_eq!(compact_panels(80).len(), 3);
        assert_eq!(compact_panels(50).len(), 2);
        assert_eq!(compact_panels(40).len(), 1);
    }

    #[test]
    fn panels_layout_scales_with_terminal() {
        let rect = |w, h| Rect::new(0, 0, w, h);

        assert_eq!(panels_layout(rect(120, 40), LayoutMode::Auto), (true, 22));
        assert_eq!(
            panels_layout(Rect::new(0, 0, 120, 30), LayoutMode::Auto),
            (false, 9)
        );
        assert_eq!(
            panels_layout(Rect::new(0, 0, 80, 24), LayoutMode::Auto),
            (false, 9)
        );
        assert_eq!(
            panels_layout(Rect::new(0, 0, 40, 12), LayoutMode::Auto),
            (false, 0)
        );
    }

    #[test]
    fn compact_mode_never_uses_the_grid() {
        assert_eq!(
            panels_layout(Rect::new(0, 0, 120, 40), LayoutMode::Compact),
            (false, 9)
        );
    }

    #[test]
    fn grid_mode_fits_smaller_terminals() {
        assert_eq!(
            panels_layout(Rect::new(0, 0, 120, 30), LayoutMode::Grid),
            (true, 22)
        );
        assert_eq!(
            panels_layout(Rect::new(0, 0, 120, 24), LayoutMode::Grid),
            (false, 9)
        );
    }

    #[test]
    fn header_shrinks_on_small_terminals() {
        assert_eq!(header_height(Rect::new(0, 0, 120, 40)), 3);
        assert_eq!(header_height(Rect::new(0, 0, 80, 24)), 1);
        assert_eq!(header_height(Rect::new(0, 0, 40, 3)), 0);
    }
}
