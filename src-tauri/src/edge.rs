//! Parking the card against a screen edge.
//!
//! When the user lets go of the card close to a monitor edge, it shrinks to a
//! small tab and sits flush against that edge; clicking the tab restores the
//! size and position it had. The geometry lives here as plain integers so it can
//! be reasoned about (and tested) without a window.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

/// Which monitor edge the tab is parked on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
    Top,
}

/// How close counts as "against the edge", in physical pixels.
///
/// Deliberately small: a card left 40 px from the edge is still a card, and
/// collapsing there would feel like the app grabbed the window.
pub const SNAP: i32 = 18;

/// Logical size of the folded square on every edge.
pub const TAB_SIZE: f64 = 32.0;

/// What the card looked like before it was folded away.
#[derive(Debug, Clone, Copy)]
pub struct Parked {
    pub side: Side,
    pub tab: Rect,
    pub width: u32,
    pub height: u32,
}

/// Managed state: `Some` while the window is folded into a tab.
#[derive(Default)]
pub struct EdgeState {
    parked: Mutex<Option<Parked>>,
    revision: AtomicU64,
    edge_preview: AtomicBool,
    /// Until this instant, a move near an edge is treated as ours, not the
    /// user's: unfolding resizes and moves the window, and those events would
    /// otherwise fold it straight back.
    quiet_until: Mutex<Option<std::time::Instant>>,
}

impl EdgeState {
    pub fn parked(&self) -> Option<Parked> {
        *self.parked.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn set(&self, parked: Option<Parked>) {
        *self.parked.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = parked;
        self.edge_preview.store(parked.is_none(), Ordering::Relaxed);
        self.revision.fetch_add(1, Ordering::Relaxed);
    }

    pub fn update_tab(&self, tab: Rect) {
        if let Some(parked) = self.parked.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            parked.tab = tab;
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::Relaxed)
    }

    pub fn is_edge_preview(&self) -> bool {
        self.edge_preview.load(Ordering::Relaxed)
    }

    pub fn end_edge_preview(&self) {
        self.edge_preview.store(false, Ordering::Relaxed);
    }

    pub fn is_parked(&self) -> bool {
        self.parked
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_some()
    }

    /// Arm the "ignore edge proximity" window used right after unfolding.
    pub fn suppress_for(&self, duration: std::time::Duration) {
        *self
            .quiet_until
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) =
            Some(std::time::Instant::now() + duration);
    }

    pub fn is_suppressed(&self) -> bool {
        let guard = self
            .quiet_until
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.is_some_and(|until| std::time::Instant::now() < until)
    }
}

/// Coalesce move/resize bursts without losing a move when resize arrives last.
#[derive(Default)]
pub struct GeometryDebounce {
    generation: u64,
    moved: bool,
}

impl GeometryDebounce {
    pub fn record(&mut self, moved: bool) -> u64 {
        self.generation += 1;
        self.moved |= moved;
        self.generation
    }

    pub fn is_current(&self, generation: u64) -> bool {
        generation == self.generation
    }

    pub fn movement(&self, generation: u64) -> Option<bool> {
        self.is_current(generation).then_some(self.moved)
    }

    /// A deferred attempt leaves the movement available to this generation,
    /// or to a newer geometry event that arrives before the retry.
    pub fn finish(&mut self, generation: u64, deferred: bool) -> bool {
        if !self.is_current(generation) {
            return false;
        }
        if deferred {
            return self.moved;
        }
        self.moved = false;
        false
    }
}

/// A rectangle in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn right(&self) -> i32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.height
    }
}

/// Which edge, if any, the card is close enough to park on.
///
/// The gap between the card and the edge is what matters, not the distance
/// between their centres, so a card wider than the screen still behaves.
pub fn side_for(card: Rect, monitor: Rect) -> Option<Side> {
    // A card can extend past an edge when the pointer reaches the screen edge.
    // A completely off-screen card is not a candidate for that monitor.
    let vertical_overlap = card.bottom() > monitor.y && card.y < monitor.bottom();
    let horizontal_overlap = card.right() > monitor.x && card.x < monitor.right();
    // Rank by actual distance to the edge, including overshoot. For an
    // exact tie prefer top, then left, then right.
    let candidates = [
        (Side::Top, card.y - monitor.y, horizontal_overlap && card.bottom() > monitor.y),
        (Side::Left, card.x - monitor.x, vertical_overlap && card.right() > monitor.x),
        (Side::Right, monitor.right() - card.right(), vertical_overlap && card.x < monitor.right()),
    ];
    let mut best: Option<(Side, i32)> = None;
    for (side, gap, overlaps) in candidates {
        if overlaps && gap <= SNAP {
            let distance = gap.abs();
            if best.map_or(true, |(_, current)| distance < current) {
                best = Some((side, distance));
            }
        }
    }
    best.map(|(side, _)| side)
}

/// A square on the work-area edge, centred on the supplied point along
/// that edge. The other coordinate stays fixed even if the pointer strays.
pub fn tab_rect(side: Side, work: Rect, center: i32, scale: f64) -> Rect {
    let size = (TAB_SIZE * scale).round() as i32;
    edge_rect(side, work, center, size, size)
}

/// Restore the original card size around the current tab position.
pub fn expanded_rect(parked: Parked, work: Rect, scale: f64) -> Rect {
    edge_rect(
        parked.side,
        work,
        along_center(parked.side, parked.tab),
        (f64::from(parked.width) * scale).round() as i32,
        (f64::from(parked.height) * scale).round() as i32,
    )
}

pub fn along_center(side: Side, rect: Rect) -> i32 {
    match side {
        Side::Top => rect.x + rect.width / 2,
        _ => rect.y + rect.height / 2,
    }
}

fn edge_rect(side: Side, work: Rect, center: i32, width: i32, height: i32) -> Rect {
    let right = (work.right() - width).max(work.x);
    let bottom = (work.bottom() - height).max(work.y);
    match side {
        Side::Left | Side::Right => Rect {
            x: if side == Side::Left { work.x } else { right },
            y: center.saturating_sub(height / 2).clamp(work.y, bottom),
            width, height,
        },
        Side::Top => Rect {
            x: center.saturating_sub(width / 2).clamp(work.x, right),
            y: work.y,
            width, height,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONITOR: Rect = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };

    fn card(x: i32, y: i32) -> Rect {
        Rect {
            x,
            y,
            width: 760,
            height: 440,
        }
    }

    #[test]
    fn overlap_at_each_supported_edge_counts_as_near() {
        assert_eq!(side_for(card(-300, 300), MONITOR), Some(Side::Left));
        assert_eq!(side_for(card(1500, 300), MONITOR), Some(Side::Right));
        assert_eq!(side_for(card(400, -200), MONITOR), Some(Side::Top));
        assert_eq!(side_for(card(19, 300), MONITOR), None);
        assert_eq!(side_for(card(-800, 300), MONITOR), None);
    }

    #[test]
    fn move_is_retained_when_resize_finishes_the_burst() {
        let mut pending = GeometryDebounce::default();
        let old = pending.record(true);
        let latest = pending.record(false);
        assert_eq!(pending.movement(old), None);
        assert_eq!(pending.movement(latest), Some(true));
        assert!(!pending.finish(latest, false));
        let resize_only = pending.record(false);
        assert_eq!(pending.movement(resize_only), Some(false));
    }

    #[test]
    fn press_after_dispatch_retries_without_another_move() {
        let mut pending = GeometryDebounce::default();
        let generation = pending.record(true);

        assert_eq!(pending.movement(generation), Some(true));
        assert!(pending.finish(generation, true)); // UI finds the button held
        assert_eq!(pending.movement(generation), Some(true)); // released, no new event
        assert!(!pending.finish(generation, false));
        assert_eq!(pending.movement(generation), Some(false));
    }

    #[test]
    fn newer_resize_inherits_deferred_move_and_invalidates_old_worker() {
        let mut pending = GeometryDebounce::default();
        let old = pending.record(true);
        assert!(pending.finish(old, true));
        let latest = pending.record(false);

        assert_eq!(pending.movement(old), None);
        assert!(!pending.finish(old, true));
        assert!(!pending.finish(old, false));
        assert_eq!(pending.movement(latest), Some(true));
    }

    #[test]
    fn exact_top_beats_overshooting_both_side_edges() {
        let wide = Rect { x: -100, y: 0, width: 2100, height: 600 };
        assert_eq!(side_for(wide, MONITOR), Some(Side::Top));
    }

    #[test]
    fn flush_left_parks_left() {
        assert_eq!(side_for(card(0, 300), MONITOR), Some(Side::Left));
        assert_eq!(side_for(card(6, 300), MONITOR), Some(Side::Left));
    }

    #[test]
    fn a_card_in_the_middle_is_left_alone() {
        assert_eq!(side_for(card(400, 300), MONITOR), None);
    }

    #[test]
    fn a_card_overlapping_the_right_edge_parks_right() {
        assert_eq!(side_for(card(1160, 300), MONITOR), Some(Side::Right));
    }

    #[test]
    fn square_tab_stays_on_work_area_edge_at_each_scale() {
        let work = Rect { x: -1900, y: -10, width: 1900, height: 1040 };
        for (scale, size) in [(1.0, 32), (1.5, 48), (2.0, 64)] {
            for side in [Side::Left, Side::Right, Side::Top] {
                let start = tab_rect(side, work, i32::MIN, scale);
                let end = tab_rect(side, work, i32::MAX, scale);
                for tab in [start, end] {
                    assert_eq!((tab.width, tab.height), (size, size));
                    assert!(tab.x >= work.x && tab.right() <= work.right());
                    assert!(tab.y >= work.y && tab.bottom() <= work.bottom());
                    match side {
                        Side::Left => assert_eq!(tab.x, work.x),
                        Side::Right => assert_eq!(tab.right(), work.right()),
                        Side::Top => assert_eq!(tab.y, work.y),
                    }
                }
                assert_ne!(along_center(side, start), along_center(side, end));
            }
        }
    }

    #[test]
    fn expansion_follows_moved_tab_and_preserves_card_dimensions() {
        for side in [Side::Left, Side::Right, Side::Top] {
            let parked = |center| Parked {
                side, tab: tab_rect(side, MONITOR, center, 1.5), width: 400, height: 250,
            };
            let moved = expanded_rect(parked(600), MONITOR, 1.5);
            assert_eq!((moved.width, moved.height), (600, 375));
            assert_eq!(along_center(side, moved), 600);
            let corner = expanded_rect(parked(10_000), MONITOR, 1.5);
            assert!(corner.right() <= MONITOR.right() && corner.bottom() <= MONITOR.bottom());
        }
    }
}
