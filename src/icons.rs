//! Tray icons, rendered from an embedded SVG in the color of the status.

use ksni::Icon;
use resvg::tiny_skia;

use crate::tray::Status;

/// Monitor outline. `__FILL__` is replaced with the status color.
const MONITOR_SVG: &str = include_str!("../assets/monitor.svg");

/// Icon sizes reported to the panel. The panel picks the best fit.
const SIZES: &[u32] = &[22, 44];

/// Prerendered icons for all states.
pub struct Icons {
    not_registered: Vec<Icon>,
    connected: Vec<Icon>,
    error: Vec<Icon>,
}

impl Icons {
    /// Render the icons of all states in all sizes.
    pub fn new() -> Self {
        Self {
            not_registered: render_all("#808080"),
            connected: render_all("#379E4A"),
            error: render_all("#D13D3D"),
        }
    }

    /// Icons for the given status.
    pub fn get(&self, status: Status) -> Vec<Icon> {
        match status {
            Status::NotRegistered => self.not_registered.clone(),
            Status::Connected => self.connected.clone(),
            Status::Error => self.error.clone(),
        }
    }
}

/// Render the icon in one color in all sizes.
fn render_all(color: &str) -> Vec<Icon> {
    let svg = MONITOR_SVG.replace("__FILL__", color);
    let tree =
        usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("embedded SVG is valid");
    SIZES.iter().map(|&size| render(&tree, size)).collect()
}

/// Rasterise the SVG and convert it to the ARGB32 layout that ksni expects.
fn render(tree: &usvg::Tree, size: u32) -> Icon {
    let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("pixmap alloc");
    let scale = size as f32 / tree.size().width();
    resvg::render(
        tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let mut data = pixmap.take_demultiplied();
    for px in data.chunks_exact_mut(4) {
        px.rotate_right(1);
    }
    Icon {
        width: size as i32,
        height: size as i32,
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_all_states() {
        let icons = Icons::new();
        for status in [Status::NotRegistered, Status::Connected, Status::Error] {
            let set = icons.get(status);
            assert_eq!(set.len(), SIZES.len());
            for icon in set {
                assert_eq!(icon.data.len(), (icon.width * icon.height * 4) as usize);
                assert!(icon.data.iter().any(|b| *b != 0));
            }
        }
    }
}
