//! Fixed geometry of the skin spritesheet.
//!
//! A skin is now a single `skin.png` — every faceplate element is a named
//! rectangle inside it, so a designer edits one file. These constants are the
//! contract between the generator (`genskin`, which paints each region) and the
//! app (`ui::skin`, which slices each region back out). Change them in one place
//! and both sides stay in sync.
//!
//! Layout (1536 × 1280):
//!   top band  y 0..512  : CASE 512², BEZEL 384², AVATAR 256², BG 512×256
//!   caps grid y 512..1280: 5 columns of 256² cells, index order
//!                          sample pads 0..11, then REC, BACK, CYCLE.

#[derive(Clone, Copy)]
pub struct Sprite {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

pub const ATLAS_W: u32 = 1536;
pub const ATLAS_H: u32 = 1280;

pub const CASE: Sprite = Sprite { x: 0, y: 0, w: 512, h: 512 };
pub const BEZEL: Sprite = Sprite { x: 512, y: 0, w: 384, h: 384 };
pub const AVATAR: Sprite = Sprite { x: 896, y: 0, w: 256, h: 256 };
pub const BG: Sprite = Sprite { x: 896, y: 256, w: 512, h: 256 };

/// Nine-slice corner inset for the case/bezel, in that sprite's own pixels.
/// Also the element's on-screen frame thickness (see `ui::skin::draw_nine`), so
/// the visualizer opening lines up exactly with the drawn bezel frame.
pub const CASE_INSET: f32 = 44.0;
pub const BEZEL_INSET: f32 = 26.0;

pub const CAP: u32 = 256;
pub const CAP_COLS: u32 = 5;
pub const CAP_Y0: u32 = 512;

pub const REC_CAP: usize = 12;
pub const BACK_CAP: usize = 13;
pub const CYCLE_CAP: usize = 14;
pub const NUM_CAPS: usize = 15;

/// The `i`-th key cap: sample pads `0..12`, then REC/BACK/CYCLE.
pub const fn cap(i: usize) -> Sprite {
    let i = i as u32;
    Sprite {
        x: (i % CAP_COLS) * CAP,
        y: CAP_Y0 + (i / CAP_COLS) * CAP,
        w: CAP,
        h: CAP,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_stay_inside_the_atlas() {
        for i in 0..NUM_CAPS {
            let s = cap(i);
            assert!(s.x + s.w <= ATLAS_W, "cap {i} x overflow");
            assert!(s.y + s.h <= ATLAS_H, "cap {i} y overflow");
        }
        // Top-band sprites too.
        for s in [CASE, BEZEL, AVATAR, BG] {
            assert!(s.x + s.w <= ATLAS_W && s.y + s.h <= ATLAS_H);
        }
    }
}
