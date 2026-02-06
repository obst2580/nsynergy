use serde::{Deserialize, Serialize};

/// Describes a display's geometry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayInfo {
    /// Display identifier (0 = primary)
    pub id: u32,
    /// Origin X in virtual screen coordinates
    pub x: i32,
    /// Origin Y in virtual screen coordinates
    pub y: i32,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Scale factor (e.g., 2.0 for Retina)
    pub scale: f64,
}

impl DisplayInfo {
    /// Right edge X coordinate (exclusive).
    pub fn right(&self) -> i32 {
        self.x + self.width as i32
    }

    /// Bottom edge Y coordinate (exclusive).
    pub fn bottom(&self) -> i32 {
        self.y + self.height as i32
    }

    /// Checks whether a point is inside this display.
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }
}

/// Which edge of the screen the cursor has reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScreenEdge {
    Left,
    Right,
    Top,
    Bottom,
}

/// Detects whether a mouse position is at a screen edge.
///
/// - `display`: the display geometry
/// - `x`, `y`: cursor position in virtual screen coordinates
/// - `threshold`: how many pixels from the edge triggers detection
///
/// Returns `Some(edge)` if the cursor is within `threshold` pixels of an edge,
/// or `None` if it is not near any edge.
pub fn detect_edge(display: &DisplayInfo, x: i32, y: i32, threshold: u32) -> Option<ScreenEdge> {
    let t = threshold as i32;

    if x <= display.x + t && y >= display.y && y < display.bottom() {
        return Some(ScreenEdge::Left);
    }
    if x >= display.right() - t - 1 && y >= display.y && y < display.bottom() {
        return Some(ScreenEdge::Right);
    }
    if y <= display.y + t && x >= display.x && x < display.right() {
        return Some(ScreenEdge::Top);
    }
    if y >= display.bottom() - t - 1 && x >= display.x && x < display.right() {
        return Some(ScreenEdge::Bottom);
    }

    None
}

/// Maps a cursor position from one display's coordinate space to another.
///
/// Preserves relative position along the shared edge.
/// For example, if the cursor exits the right edge of `from` at 50% height,
/// it enters the left edge of `to` at 50% height.
pub fn map_position(
    from: &DisplayInfo,
    to: &DisplayInfo,
    edge: ScreenEdge,
    x: i32,
    y: i32,
) -> (i32, i32) {
    match edge {
        ScreenEdge::Right | ScreenEdge::Left => {
            // Preserve vertical ratio
            let ratio = if from.height > 0 {
                (y - from.y) as f64 / from.height as f64
            } else {
                0.5
            };
            let new_y = to.y + (ratio * to.height as f64) as i32;
            let new_x = match edge {
                ScreenEdge::Right => to.x + 1,
                ScreenEdge::Left => to.right() - 2,
                _ => unreachable!(),
            };
            (new_x, new_y.clamp(to.y, to.bottom() - 1))
        }
        ScreenEdge::Top | ScreenEdge::Bottom => {
            // Preserve horizontal ratio
            let ratio = if from.width > 0 {
                (x - from.x) as f64 / from.width as f64
            } else {
                0.5
            };
            let new_x = to.x + (ratio * to.width as f64) as i32;
            let new_y = match edge {
                ScreenEdge::Bottom => to.y + 1,
                ScreenEdge::Top => to.bottom() - 2,
                _ => unreachable!(),
            };
            (new_x.clamp(to.x, to.right() - 1), new_y)
        }
    }
}

/// Queries the primary display info using platform APIs.
///
/// Returns a hardcoded default if the query fails. This will be replaced
/// with actual platform calls when the GUI layer (Tauri) provides display info.
pub fn primary_display() -> DisplayInfo {
    // TODO: Replace with actual platform query (CoreGraphics on macOS, WinAPI on Windows)
    DisplayInfo {
        id: 0,
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
        scale: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_display(x: i32, y: i32, w: u32, h: u32) -> DisplayInfo {
        DisplayInfo {
            id: 0,
            x,
            y,
            width: w,
            height: h,
            scale: 1.0,
        }
    }

    #[test]
    fn display_right_bottom() {
        let d = make_display(100, 200, 1920, 1080);
        assert_eq!(d.right(), 2020);
        assert_eq!(d.bottom(), 1280);
    }

    #[test]
    fn display_contains_inside() {
        let d = make_display(0, 0, 1920, 1080);
        assert!(d.contains(960, 540));
        assert!(d.contains(0, 0));
        assert!(d.contains(1919, 1079));
    }

    #[test]
    fn display_contains_outside() {
        let d = make_display(0, 0, 1920, 1080);
        assert!(!d.contains(-1, 540));
        assert!(!d.contains(1920, 540));
        assert!(!d.contains(960, 1080));
        assert!(!d.contains(960, -1));
    }

    #[test]
    fn detect_edge_left() {
        let d = make_display(0, 0, 1920, 1080);
        assert_eq!(detect_edge(&d, 0, 500, 2), Some(ScreenEdge::Left));
        assert_eq!(detect_edge(&d, 1, 500, 2), Some(ScreenEdge::Left));
        assert_eq!(detect_edge(&d, 2, 500, 2), Some(ScreenEdge::Left));
        assert_eq!(detect_edge(&d, 3, 500, 2), None);
    }

    #[test]
    fn detect_edge_right() {
        let d = make_display(0, 0, 1920, 1080);
        assert_eq!(detect_edge(&d, 1919, 500, 2), Some(ScreenEdge::Right));
        assert_eq!(detect_edge(&d, 1918, 500, 2), Some(ScreenEdge::Right));
        assert_eq!(detect_edge(&d, 1917, 500, 2), Some(ScreenEdge::Right));
        assert_eq!(detect_edge(&d, 1916, 500, 2), None);
    }

    #[test]
    fn detect_edge_top() {
        let d = make_display(0, 0, 1920, 1080);
        assert_eq!(detect_edge(&d, 960, 0, 2), Some(ScreenEdge::Top));
        assert_eq!(detect_edge(&d, 960, 2, 2), Some(ScreenEdge::Top));
        assert_eq!(detect_edge(&d, 960, 3, 2), None);
    }

    #[test]
    fn detect_edge_bottom() {
        let d = make_display(0, 0, 1920, 1080);
        assert_eq!(detect_edge(&d, 960, 1079, 2), Some(ScreenEdge::Bottom));
        assert_eq!(detect_edge(&d, 960, 1078, 2), Some(ScreenEdge::Bottom));
        assert_eq!(detect_edge(&d, 960, 1077, 2), Some(ScreenEdge::Bottom));
        assert_eq!(detect_edge(&d, 960, 1076, 2), None);
    }

    #[test]
    fn detect_edge_center_returns_none() {
        let d = make_display(0, 0, 1920, 1080);
        assert_eq!(detect_edge(&d, 960, 540, 2), None);
    }

    #[test]
    fn detect_edge_with_offset_display() {
        let d = make_display(1920, 0, 1920, 1080);
        assert_eq!(detect_edge(&d, 1920, 500, 2), Some(ScreenEdge::Left));
        assert_eq!(detect_edge(&d, 3839, 500, 2), Some(ScreenEdge::Right));
    }

    #[test]
    fn map_position_right_to_left_preserves_y_ratio() {
        let from = make_display(0, 0, 1920, 1080);
        let to = make_display(1920, 0, 2560, 1440);

        // Cursor at 50% height on right edge
        let (new_x, new_y) = map_position(&from, &to, ScreenEdge::Right, 1919, 540);
        assert_eq!(new_x, to.x + 1); // enters left side of target
        assert_eq!(new_y, 720); // 50% of 1440
    }

    #[test]
    fn map_position_left_to_right() {
        let from = make_display(1920, 0, 2560, 1440);
        let to = make_display(0, 0, 1920, 1080);

        // Cursor at 25% height on left edge
        let (new_x, new_y) = map_position(&from, &to, ScreenEdge::Left, 1920, 360);
        assert_eq!(new_x, to.right() - 2); // enters right side of target
        assert_eq!(new_y, 270); // 25% of 1080
    }

    #[test]
    fn map_position_bottom_to_top_preserves_x_ratio() {
        let from = make_display(0, 0, 1920, 1080);
        let to = make_display(0, 1080, 1920, 1080);

        let (new_x, new_y) = map_position(&from, &to, ScreenEdge::Bottom, 960, 1079);
        assert_eq!(new_y, to.y + 1);
        assert_eq!(new_x, 960); // 50% preserved
    }

    #[test]
    fn map_position_clamps_to_target() {
        let from = make_display(0, 0, 1920, 1080);
        let to = make_display(1920, 200, 800, 600);

        // Cursor near bottom of `from`, ratio close to 1.0
        let (_, new_y) = map_position(&from, &to, ScreenEdge::Right, 1919, 1079);
        assert!(new_y < to.bottom());
        assert!(new_y >= to.y);
    }

    #[test]
    fn primary_display_returns_valid_info() {
        let d = primary_display();
        assert!(d.width > 0);
        assert!(d.height > 0);
        assert!(d.scale > 0.0);
    }
}
