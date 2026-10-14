//! The pen line: one stroke that gets heavier from the left to the right,
//! with stops along it. Ink fills the line up to a value, and the rest stays
//! as an impression in the paper. The model picker uses it for the thinking
//! level (design file, page "Model picker · Thinking").
//!
//! The geometry is in design units: the line is 352 wide, and the canvas
//! scales all of it to its own width.

use crate::theme::Colors;
use gpui_kit::*;

/// The width of the design.
const WIDTH: f32 = 352.;
/// Where the line starts and ends.
const START: f32 = 4.;
const END: f32 = 346.;
/// The first and the last stop.
const FIRST_STOP: f32 = 24.;
const LAST_STOP: f32 = 344.;
/// The height that the line and its spatter need above and below its center.
const HALF_HEIGHT: f32 = 22.;
/// The height of the row of ticks under the line.
const TICKS: f32 = 10.;

/// The height of a [`pen_line`] at the width of the design.
pub const PEN_LINE_HEIGHT: f32 = HALF_HEIGHT * 2. + TICKS;

/// The height of a [`pen_line`] that is `width` wide.
pub fn pen_line_height(width: f32) -> f32 {
    (PEN_LINE_HEIGHT * width / WIDTH).ceil()
}

/// Specks of ink near the heavy part of the line: x, the offset from the
/// center of the line, and the radius. A speck shows when the ink reaches it.
/// The places are fixed, so the line looks the same each time.
const SPECKS: [(f32, f32, f32); 17] = [
    (150., -5., 0.45),
    (172., 6., 0.55),
    (203., -7., 0.6),
    (213., -9.5, 0.4),
    (236., 8., 0.7),
    (249., 10.5, 0.4),
    (262., -9., 0.8),
    (270., -12.5, 0.45),
    (289., 10., 0.9),
    (296., 13.5, 0.45),
    (309., -11., 0.5),
    (318., -10., 1.0),
    (325., -14., 0.5),
    (333., 11.5, 0.75),
    (341., 15., 0.45),
    (350., -7., 0.6),
    (351., 9., 0.4),
];

/// The place of stop `index` of `count` on the line, in design units.
pub fn stop_x(index: usize, count: usize) -> f32 {
    if count < 2 {
        return LAST_STOP;
    }
    FIRST_STOP + (LAST_STOP - FIRST_STOP) * index as f32 / (count - 1) as f32
}

/// The value (0.0 at the first stop, 1.0 at the last) for a pointer at `x`
/// pixels from the left edge of a line that is `width` pixels wide.
pub fn value_at(x: f32, width: f32) -> f32 {
    let design = x / width.max(1.) * WIDTH;
    ((design - FIRST_STOP) / (LAST_STOP - FIRST_STOP)).clamp(0., 1.)
}

/// The stop nearest to `value`.
pub fn nearest_stop(value: f32, count: usize) -> usize {
    if count < 2 {
        return 0;
    }
    (value.clamp(0., 1.) * (count - 1) as f32).round() as usize
}

/// The value of stop `index` of `count`.
pub fn stop_value(index: usize, count: usize) -> f32 {
    if count < 2 { 1. } else { index as f32 / (count - 1) as f32 }
}

/// The center of the line at `x`, as an offset from the middle. A slow wave.
fn center(x: f32) -> f32 {
    -3. * (std::f32::consts::TAU * (x - START) / 176.).sin()
}

/// Half of the width of the line at `x`: a hairline at the start, heavy at
/// the end. A downstroke is a little heavier than an upstroke, as with a nib.
fn half_width(x: f32) -> f32 {
    let t = ((x - START) / (END - START)).clamp(0., 1.);
    let slope = center(x + 0.5) - center(x - 0.5);
    (0.45 + 6.6 * t.powf(1.5)) * (1. + 2.2 * slope)
}

/// Where the ink ends for `value`. The last stop takes the ink to the end of the line.
fn ink_end(value: f32) -> f32 {
    if value >= 0.999 { END } else { FIRST_STOP + (LAST_STOP - FIRST_STOP) * value.clamp(0., 1.) }
}

/// The outline of the line from its start to `end`, with `grow` added to its
/// half width. The end is flat and a little oblique, as a nib leaves it.
fn outline(end: f32, grow: f32) -> Vec<(f32, f32)> {
    let mut xs = Vec::new();
    let mut x = START;
    while x < end - 1.5 {
        xs.push(x);
        x += 3.;
    }
    xs.push(end);
    let slant = (half_width(end) * 0.35).min(1.3);
    let last = xs.len() - 1;
    let mut points: Vec<(f32, f32)> = xs
        .iter()
        .enumerate()
        .map(|(i, &x)| (if i == last { x + slant + grow } else { x }, center(x) - half_width(x) - grow))
        .collect();
    points.extend(
        xs.iter().enumerate().rev().map(|(i, &x)| (if i == last { x - slant + grow } else { x }, center(x) + half_width(x) + grow)),
    );
    points
}

/// The pen line.
///
/// - `value`: where the ink ends, 0.0 at the first stop and 1.0 at the last.
/// - `stops`: the number of stops. `marked` is the stop with the ink tick.
/// - `wet`: the user holds the line. The cut then has a pool of wet ink.
pub fn pen_line(value: f32, stops: usize, marked: usize, wet: bool, c: &Colors) -> Canvas<()> {
    let (ink, impression, edge, pencil) = (c.ink, c.deboss, c.deboss_shade, c.pencil);
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let scale = bounds.size.width.as_f32() / WIDTH;
            let mid = bounds.origin.y + px(HALF_HEIGHT * scale);
            let at = |x: f32, y: f32| point(bounds.origin.x + px(x * scale), mid + px(y * scale));
            let fill_shape = |points: &[(f32, f32)], color: Hsla, window: &mut Window| {
                let mut pb = PathBuilder::fill();
                pb.move_to(at(points[0].0, points[0].1));
                for &(x, y) in &points[1..] {
                    pb.line_to(at(x, y));
                }
                pb.close();
                if let Ok(path) = pb.build() {
                    window.paint_path(path, color);
                }
            };
            let dot = |x: f32, y: f32, r: f32, color: Hsla, window: &mut Window| {
                let (center, r) = (at(x, y), r * scale);
                window.paint_quad(
                    fill(Bounds { origin: point(center.x - px(r), center.y - px(r)), size: size(px(r * 2.), px(r * 2.)) }, color)
                        .corner_radii(px(r)),
                );
            };

            let end = ink_end(value);
            // The impression: the whole line, pressed into the paper.
            if end < END {
                fill_shape(&outline(END, 0.5), edge, window);
                fill_shape(&outline(END, 0.), impression, window);
            }
            fill_shape(&outline(end, 0.), ink, window);
            dot(START + 0.6, center(START), 1.1, ink, window);

            // A heavy line throws specks, and the tines scratch past the cut.
            for (x, dy, r) in SPECKS {
                if x <= end + 7. {
                    dot(x, center(x) + dy, r, ink, window);
                }
            }
            let heavy = half_width(end);
            if heavy > 1.8 {
                let reach = (end - START) / (END - START);
                for (from, length, lift) in [(-0.45, 1.5 + reach * 5.2, -0.5), (0.4, 1. + reach * 3.6, 0.6)] {
                    let mut pb = PathBuilder::stroke(px(0.55 * scale));
                    let y = center(end) + heavy * from;
                    pb.move_to(at(end + 0.7, y));
                    pb.line_to(at(end + 0.7 + length, y + lift));
                    if let Ok(path) = pb.build() {
                        window.paint_path(path, ink);
                    }
                }
            }
            if wet {
                dot(end, center(end), heavy + 1.6, ink, window);
            }

            // The stops.
            let tick_top = HALF_HEIGHT + 1.;
            for i in 0..stops {
                let x = bounds.origin.x + px(stop_x(i, stops) * scale);
                let (w, h, y, color) = if i == marked { (1.6, 8., tick_top - 2., ink) } else { (1., 4., tick_top, pencil) };
                window.paint_quad(
                    fill(Bounds { origin: point(x - px(w / 2.), mid + px(y * scale)), size: size(px(w), px(h * scale)) }, color)
                        .corner_radii(px(w / 2.)),
                );
            }
        },
    )
}

#[cfg(test)]
mod tests {
    // No glob import: `gpui_kit` has a `test` attribute of its own.
    use super::{END, HALF_HEIGHT, SPECKS, START, center, half_width, ink_end, nearest_stop, outline, stop_value, stop_x, value_at};

    #[test]
    fn stops_spread_from_the_first_to_the_last() {
        assert_eq!((stop_x(0, 5), stop_x(2, 5), stop_x(4, 5)), (24., 184., 344.));
        assert_eq!(stop_x(0, 1), 344., "one level is the whole line");
        assert_eq!((stop_value(0, 5), stop_value(4, 5), stop_value(0, 1)), (0., 1., 1.));
    }

    #[test]
    fn a_pointer_gives_the_nearest_stop() {
        // The line is twice the design width.
        let width = 704.;
        assert_eq!(value_at(0., width), 0., "left of the first stop");
        assert_eq!(value_at(704., width), 1., "right of the last stop");
        let value = value_at(2. * 184., width);
        assert!((value - 0.5).abs() < 0.001);
        assert_eq!(nearest_stop(value, 5), 2);
        assert_eq!(nearest_stop(value_at(2. * 230., width), 5), 3, "past the middle of two stops");
        assert_eq!(nearest_stop(0.7, 1), 0);
    }

    #[test]
    fn the_line_gets_heavier_and_stays_in_its_box() {
        assert!(half_width(START) < 0.6, "the start is a hairline");
        assert!(half_width(stop_x(1, 5)) < half_width(stop_x(2, 5)));
        assert!(half_width(stop_x(2, 5)) < half_width(stop_x(3, 5)));
        assert!(half_width(END) > 4.5);
        let reach = outline(END, 0.5).iter().map(|&(_, y)| y.abs()).fold(0., f32::max);
        assert!(reach < HALF_HEIGHT, "the line leaves its box: {reach}");
        let specks = SPECKS.iter().map(|&(x, dy, r)| (center(x) + dy).abs() + r).fold(0., f32::max);
        assert!(specks < HALF_HEIGHT, "a speck leaves the box: {specks}");
    }

    #[test]
    fn a_narrow_line_is_less_tall() {
        assert_eq!(super::pen_line_height(352.), super::PEN_LINE_HEIGHT);
        assert_eq!(super::pen_line_height(176.), super::PEN_LINE_HEIGHT / 2.);
    }

    #[test]
    fn the_ink_ends_at_the_value() {
        assert_eq!(ink_end(0.), 24.);
        assert_eq!(ink_end(0.5), 184.);
        assert_eq!(ink_end(1.), END, "the last stop inks the whole line");
        let points = outline(184., 0.);
        let right = points.iter().map(|&(x, _)| x).fold(0., f32::max);
        assert!((184.0..186.).contains(&right), "the cut is at the value: {right}");
        // A light line has no specks: the first one is past the second of five stops.
        assert!(SPECKS.iter().all(|&(x, _, _)| x > stop_x(1, 5) + 7.));
    }
}
