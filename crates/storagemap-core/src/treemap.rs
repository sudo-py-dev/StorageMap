//! Squarified treemap layout (Bruls, Huizing & van Wijk, 2000).

/// Axis-aligned rectangle in layout units.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn area(&self) -> f32 {
        self.w * self.h
    }
}

/// Lays out `weights` (sorted largest first, all > 0) inside `area`, keeping
/// tiles as close to square as possible. Returns one rect per weight.
pub fn squarify(weights: &[f64], area: Rect) -> Vec<Rect> {
    let total: f64 = weights.iter().sum();
    if weights.is_empty() || total <= 0.0 || area.area() <= 0.0 {
        return vec![
            Rect {
                x: area.x,
                y: area.y,
                w: 0.0,
                h: 0.0
            };
            weights.len()
        ];
    }
    let scale = f64::from(area.area()) / total;
    let areas: Vec<f64> = weights.iter().map(|w| w * scale).collect();

    let mut out = Vec::with_capacity(areas.len());
    let mut free = area;
    let mut start = 0;
    while start < areas.len() {
        let side = f64::from(free.w.min(free.h));
        let mut end = start + 1;
        let mut row_sum = areas[start];
        let mut best = worst_ratio(areas[start], areas[start], row_sum, side);
        while end < areas.len() {
            let candidate = row_sum + areas[end];
            let ratio = worst_ratio(areas[start], areas[end], candidate, side);
            if ratio > best {
                break;
            }
            best = ratio;
            row_sum = candidate;
            end += 1;
        }
        free = place_row(&areas[start..end], row_sum, free, &mut out);
        start = end;
    }
    out
}

/// Worst aspect ratio of a row whose largest item is `max`, smallest `min`.
fn worst_ratio(max: f64, min: f64, sum: f64, side: f64) -> f64 {
    let (s2, sum2) = (side * side, sum * sum);
    (s2 * max / sum2).max(sum2 / (s2 * min))
}

/// Places one row along the shorter side and returns the remaining space.
fn place_row(row: &[f64], row_sum: f64, free: Rect, out: &mut Vec<Rect>) -> Rect {
    if free.w >= free.h {
        let strip = (row_sum / f64::from(free.h)) as f32;
        let mut y = free.y;
        for &a in row {
            let h = (a / f64::from(strip)) as f32;
            out.push(Rect {
                x: free.x,
                y,
                w: strip,
                h,
            });
            y += h;
        }
        Rect {
            x: free.x + strip,
            w: (free.w - strip).max(0.0),
            ..free
        }
    } else {
        let strip = (row_sum / f64::from(free.w)) as f32;
        let mut x = free.x;
        for &a in row {
            let w = (a / f64::from(strip)) as f32;
            out.push(Rect {
                x,
                y: free.y,
                w,
                h: strip,
            });
            x += w;
        }
        Rect {
            y: free.y + strip,
            h: (free.h - strip).max(0.0),
            ..free
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect {
        x: 0.0,
        y: 0.0,
        w: 600.0,
        h: 400.0,
    };

    #[test]
    fn preserves_proportional_areas() {
        let weights = [6.0, 6.0, 4.0, 3.0, 2.0, 2.0, 1.0];
        let rects = squarify(&weights, AREA);
        let total: f64 = weights.iter().sum();
        for (w, r) in weights.iter().zip(&rects) {
            let expected = (w / total) as f32 * AREA.area();
            assert!((r.area() - expected).abs() < 1.0, "{r:?} vs {expected}");
        }
    }

    #[test]
    fn stays_inside_bounds() {
        let rects = squarify(&[50.0, 30.0, 10.0, 5.0, 5.0], AREA);
        for r in rects {
            assert!(r.x >= -0.01 && r.y >= -0.01);
            assert!(r.x + r.w <= AREA.w + 0.01 && r.y + r.h <= AREA.h + 0.01);
        }
    }

    #[test]
    fn degenerate_inputs() {
        assert!(squarify(&[], AREA).is_empty());
        let zero = squarify(&[1.0], Rect { w: 0.0, ..AREA });
        assert_eq!(zero[0].area(), 0.0);
    }
}
