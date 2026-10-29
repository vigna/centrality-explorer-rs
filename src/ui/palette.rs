//! Node colors.

use egui::Color32;

use crate::graph::PALETTE_SIZE;

/// The node colors: saturated, mutually distinct when projected, with no
/// grays (gray reads as a faded curve) and no light yellows.
pub const PALETTE: [Color32; PALETTE_SIZE] = [
    Color32::from_rgb(0x1f, 0x77, 0xb4), // blue
    Color32::from_rgb(0xff, 0x7f, 0x0e), // orange
    Color32::from_rgb(0x2c, 0xa0, 0x2c), // green
    Color32::from_rgb(0xd6, 0x27, 0x28), // red
    Color32::from_rgb(0x94, 0x67, 0xbd), // purple
    Color32::from_rgb(0x8c, 0x56, 0x4b), // brown
    Color32::from_rgb(0xe3, 0x77, 0xc2), // pink
    Color32::from_rgb(0x17, 0xbe, 0xcf), // cyan
    Color32::from_rgb(0x7b, 0x41, 0x73), // plum
    Color32::from_rgb(0x39, 0x3b, 0x79), // navy
    Color32::from_rgb(0x63, 0x79, 0x39), // olive
    Color32::from_rgb(0xa6, 0x76, 0x1d), // mustard
];

/// Returns the color with the given palette index (modulo the palette size).
pub fn color(index: usize) -> Color32 {
    PALETTE[index % PALETTE_SIZE]
}

/// Returns a label color contrasting with the color with the given palette
/// index: black on light colors, white on dark ones.
pub fn label_color(index: usize) -> Color32 {
    let c = color(index);
    let luminance = 0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32;
    if luminance > 140.0 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}
