use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use gtk::prelude::*;

/// Line color matches `lyra_blue` in resources/style.css — sparklines are
/// single-series (the metric's own label already gives it identity, per
/// the project's chart guidance), so every graph uses the same accent
/// rather than inventing a new color per metric.
const LINE_RGB: (f64, f64, f64) = (
    0x5b as f64 / 255.0,
    0x8c as f64 / 255.0,
    0xff as f64 / 255.0,
);
/// Neutral gray for the reference grid — deliberately not the accent color,
/// so the grid reads as scaffolding behind the data, not a second series.
const GRID_RGB: (f64, f64, f64) = (0.5, 0.5, 0.5);
const GRID_FRACTIONS: [f64; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];

#[derive(Clone)]
pub struct Sparkline {
    pub widget: gtk::DrawingArea,
    history: Rc<RefCell<VecDeque<f64>>>,
    capacity: usize,
}

impl Sparkline {
    pub fn new(capacity: usize, fixed_max: Option<f64>) -> Self {
        let widget = gtk::DrawingArea::builder()
            .content_height(80)
            .hexpand(true)
            .vexpand(true)
            .build();
        let history = Rc::new(RefCell::new(VecDeque::with_capacity(capacity)));

        let draw_history = history.clone();
        widget.set_draw_func(move |_area, cr, width, height| {
            draw_sparkline(cr, width, height, &draw_history.borrow(), fixed_max);
        });

        Self {
            widget,
            history,
            capacity,
        }
    }

    pub fn push(&self, value: f64) {
        push_sample(
            &mut self.history.borrow_mut(),
            self.capacity,
            value.max(0.0),
        );
        self.widget.queue_draw();
    }
}

fn push_sample(history: &mut VecDeque<f64>, capacity: usize, value: f64) {
    if history.len() == capacity {
        history.pop_front();
    }
    history.push_back(value);
}

fn draw_sparkline(
    cr: &gtk::cairo::Context,
    width: i32,
    height: i32,
    history: &VecDeque<f64>,
    fixed_max: Option<f64>,
) {
    let width = f64::from(width);
    let height = f64::from(height);
    if history.len() < 2 || width <= 0.0 || height <= 0.0 {
        return;
    }

    let max = fixed_max
        .unwrap_or_else(|| history.iter().cloned().fold(0.0_f64, f64::max))
        .max(1.0);
    // Small top margin so a flat 100%/max line doesn't clip against the
    // widget edge.
    let plot_height = height - 4.0;
    let step = width / (history.len() - 1) as f64;
    let point = |i: usize, value: f64| {
        let x = step * i as f64;
        let y = 2.0 + plot_height - (value / max).min(1.0) * plot_height;
        (x, y)
    };

    // Labeled marks only when there's real room for them (the tiny
    // per-core cells skip labels but still get the plain grid lines) and
    // only for percent-style graphs (fixed_max) — an auto-scaled rate has
    // no fixed unit to print here.
    let show_labels = fixed_max.is_some() && plot_height > 40.0;
    draw_grid(cr, width, plot_height, max, show_labels);

    let (r, g, b) = LINE_RGB;

    cr.new_path();
    let (first_x, first_y) = point(0, history[0]);
    cr.move_to(first_x, first_y);
    for (i, value) in history.iter().enumerate().skip(1) {
        let (x, y) = point(i, *value);
        cr.line_to(x, y);
    }
    let (last_x, _) = point(history.len() - 1, *history.back().unwrap());
    cr.line_to(last_x, height);
    cr.line_to(first_x, height);
    cr.close_path();
    cr.set_source_rgba(r, g, b, 0.16);
    let _ = cr.fill_preserve();

    cr.new_path();
    cr.move_to(first_x, first_y);
    for (i, value) in history.iter().enumerate().skip(1) {
        let (x, y) = point(i, *value);
        cr.line_to(x, y);
    }
    cr.set_source_rgba(r, g, b, 0.95);
    cr.set_line_width(2.0);
    cr.set_line_join(gtk::cairo::LineJoin::Round);
    cr.set_line_cap(gtk::cairo::LineCap::Round);
    let _ = cr.stroke();
}

/// Horizontal reference lines at 0/25/50/75/100% of `max`, spanning the
/// full width — the same coordinate math the data line itself uses, so a
/// point on the line always lands exactly on its matching mark.
fn draw_grid(cr: &gtk::cairo::Context, width: f64, plot_height: f64, max: f64, show_labels: bool) {
    let (r, g, b) = GRID_RGB;
    cr.set_line_width(1.0);
    cr.set_font_size(9.0);
    for fraction in GRID_FRACTIONS {
        let y = 2.0 + plot_height - fraction * plot_height;
        cr.new_path();
        cr.move_to(0.0, y);
        cr.line_to(width, y);
        cr.set_source_rgba(r, g, b, 0.14);
        let _ = cr.stroke();

        if show_labels {
            let label = format!("{:.0}%", fraction * max);
            cr.set_source_rgba(r, g, b, 0.75);
            // Clamp so the 100% mark's text doesn't clip above the top
            // edge and the 0% mark's doesn't clip below the bottom.
            cr.move_to(3.0, (y + 3.0).clamp(9.0, plot_height));
            let _ = cr.show_text(&label);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_drops_oldest_sample_once_at_capacity() {
        let mut history = VecDeque::new();
        for value in [1.0, 2.0, 3.0, 4.0] {
            push_sample(&mut history, 3, value);
        }
        assert_eq!(history.into_iter().collect::<Vec<_>>(), vec![2.0, 3.0, 4.0]);
    }
}
