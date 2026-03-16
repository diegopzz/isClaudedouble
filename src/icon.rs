const ICON_DIMENSION: u32 = 32;
const CELL_SCALE: usize = 2;
const ACTIVE_FILL: [u8; 4] = [0xD2, 0x8F, 0x69, 0xFF];
const INACTIVE_FILL: [u8; 4] = [0x00, 0x00, 0x00, 0xFF];

const SPRITE_MASK: [&str; 16] = [
    "................",
    "....########....",
    "....########....",
    "....#.####.#....",
    "..############..",
    "..############..",
    "..############..",
    "...##########...",
    "...##.##.##.##..",
    "...##.##.##.##..",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconImage {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub fn active_icon() -> IconImage {
    render_icon(ACTIVE_FILL)
}

pub fn inactive_icon() -> IconImage {
    render_icon(INACTIVE_FILL)
}

fn render_icon(fill: [u8; 4]) -> IconImage {
    let width = ICON_DIMENSION as usize;
    let height = ICON_DIMENSION as usize;
    let mut rgba = vec![0; width * height * 4];

    for (row_index, row) in SPRITE_MASK.iter().enumerate() {
        for (column_index, pixel) in row.chars().enumerate() {
            if pixel != '#' {
                continue;
            }

            for y in 0..CELL_SCALE {
                for x in 0..CELL_SCALE {
                    let rendered_x = column_index * CELL_SCALE + x;
                    let rendered_y = row_index * CELL_SCALE + y;
                    let pixel_index = (rendered_y * width + rendered_x) * 4;

                    rgba[pixel_index..pixel_index + 4].copy_from_slice(&fill);
                }
            }
        }
    }

    IconImage {
        rgba,
        width: ICON_DIMENSION,
        height: ICON_DIMENSION,
    }
}

#[cfg(test)]
mod tests {
    use super::{ACTIVE_FILL, INACTIVE_FILL, active_icon, inactive_icon};

    #[test]
    fn active_and_inactive_icons_share_the_same_shape() {
        let active = active_icon();
        let inactive = inactive_icon();

        assert_eq!(active.width, inactive.width);
        assert_eq!(active.height, inactive.height);

        let active_alpha: Vec<u8> = active.rgba.iter().skip(3).step_by(4).copied().collect();
        let inactive_alpha: Vec<u8> = inactive.rgba.iter().skip(3).step_by(4).copied().collect();

        assert_eq!(active_alpha, inactive_alpha);
    }

    #[test]
    fn active_icon_uses_only_the_requested_orange_fill() {
        let icon = active_icon();
        assert_opaque_pixels_match_fill(&icon.rgba, ACTIVE_FILL);
    }

    #[test]
    fn inactive_icon_uses_only_black_fill() {
        let icon = inactive_icon();
        assert_opaque_pixels_match_fill(&icon.rgba, INACTIVE_FILL);
    }

    #[test]
    fn transparent_pixels_stay_transparent() {
        let icon = active_icon();

        assert!(
            icon.rgba
                .chunks_exact(4)
                .any(|pixel| pixel == [0, 0, 0, 0].as_slice())
        );
    }

    fn assert_opaque_pixels_match_fill(rgba: &[u8], fill: [u8; 4]) {
        for pixel in rgba.chunks_exact(4) {
            if pixel[3] == 0 {
                assert_eq!(pixel, [0, 0, 0, 0].as_slice());
            } else {
                assert_eq!(pixel, fill.as_slice());
            }
        }
    }
}
