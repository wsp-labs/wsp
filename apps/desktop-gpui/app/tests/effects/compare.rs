// SPDX-License-Identifier: AGPL-3.0-only

use std::fmt;
use std::path::Path;

use image::{Rgba, RgbaImage};

pub struct Stats {
    pub max: u8,
    pub mean: f64,
    pub p99: u8,
    pub over: usize,
    pub total: usize,
}

impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "max {}, mean {:.2}, p99 {}, over tolerance {} of {} px", self.max, self.mean, self.p99, self.over, self.total)
    }
}

/// Per-pixel max channel difference between two equally sized RGBA images; writes a diff PNG (each pixel's difference x8, clamped, as grey, with pixels over `tolerance` in red) to `diff_path`.
pub fn compare(actual: &RgbaImage, expected: &RgbaImage, tolerance: u8, diff_path: &Path) -> Stats {
    assert_eq!(
        actual.dimensions(),
        expected.dimensions(),
        "image sizes differ: actual {:?}, expected {:?}",
        actual.dimensions(),
        expected.dimensions()
    );

    let (width, height) = actual.dimensions();
    let mut diff = RgbaImage::new(width, height);
    let mut histogram = [0usize; 256];
    let mut sum = 0u64;

    for ((a, e), d) in actual.pixels().zip(expected.pixels()).zip(diff.pixels_mut()) {
        let delta = (0..3).map(|c| a[c].abs_diff(e[c])).max().unwrap_or(0);
        histogram[delta as usize] += 1;
        sum += u64::from(delta);
        *d = if delta > tolerance {
            Rgba([255, 0, 0, 255])
        } else {
            let grey = delta.saturating_mul(8);
            Rgba([grey, grey, grey, 255])
        };
    }

    let total = (width as usize) * (height as usize);
    let max = histogram.iter().rposition(|&n| n > 0).unwrap_or(0) as u8;
    let over = histogram[tolerance as usize + 1..].iter().sum();
    let p99_rank = (total * 99).div_ceil(100);
    let mut seen = 0;
    let mut p99 = 0u8;
    for (value, &n) in histogram.iter().enumerate() {
        seen += n;
        if seen >= p99_rank {
            p99 = value as u8;
            break;
        }
    }

    if let Some(parent) = diff_path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|e| panic!("creating {}: {e}", parent.display()));
    }
    diff.save(diff_path).unwrap_or_else(|e| panic!("writing {}: {e}", diff_path.display()));

    Stats { max, mean: if total == 0 { 0.0 } else { sum as f64 / total as f64 }, p99, over, total }
}
