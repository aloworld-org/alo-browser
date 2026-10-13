/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How Chrome 154 shares a flex line among its items, at a scale of one and
//! of two, against 762 lines it was measured on (ADR 0054's addendum of
//! 2026-10-13, queue item 564).
//!
//! The rule is written here as ADR 0054 states it, and checked to give every
//! one of Chrome's lines to the unit. This engine does not lay lines out by
//! it yet: the shares are `taffy`'s, decided before the boundary sees them,
//! and putting them on the grid waits for the hook ADR 0056 offers upstream
//! (queue items 501 and 502). What this engine gives today is pinned below,
//! within two units of Chrome's at every edge and on the grid, so that the build
//! which takes the hook has to change this file to say it moved.

use std::collections::BTreeMap;

use alo_box::{BoxId, build};
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::{NodeId, parse_document};
use alo_layout::{LayoutTree, NoText, Size, compute};
use alo_style::{Origin, SourcedSheet, StyleTree, USER_AGENT_STYLE_SHEET, resolve};

/// One item as written: its content-box width and padding in px, and its
/// flex factor.
struct Item {
    width: f64,
    factor: f64,
    padding: f64,
}

/// One measured line: what was written, and Chrome's left and width of each
/// item, in units of the grid at a scale of one and of two.
struct Line {
    shrinks: bool,
    width: f64,
    gap: f64,
    items: Vec<Item>,
    at_one: Vec<(i64, i64)>,
    at_two: Vec<(i64, i64)>,
}

impl Line {
    fn chrome(&self, scale: u8) -> &[(i64, i64)] {
        if scale == 1 {
            &self.at_one
        } else {
            &self.at_two
        }
    }

    /// The markup of this line: a flex container holding its items.
    fn css(&self, index: usize) -> String {
        let flex = |item: &Item| {
            if self.shrinks {
                format!("flex-shrink: {}", item.factor)
            } else {
                format!("flex-grow: {}; flex-shrink: 0", item.factor)
            }
        };
        let container = format!(
            "#l{index} {{ display: flex; width: {}px; column-gap: {}px }}\n",
            self.width, self.gap
        );
        let items = self.items.iter().enumerate().map(|(at, item)| {
            format!(
                "#l{index}i{at} {{ {}; width: {}px; padding: 0 {}px }}\n",
                flex(item),
                item.width,
                item.padding
            )
        });
        core::iter::once(container).chain(items).collect()
    }
}

fn whole(text: &str) -> Option<(i64, i64)> {
    let (left, width) = text.split_once(':')?;
    Some((left.parse().ok()?, width.parse().ok()?))
}

fn measured(text: &str) -> Option<Vec<(i64, i64)>> {
    text.split_whitespace().map(whole).collect()
}

fn lines() -> Option<Vec<Line>> {
    let text = include_str!("references/flex-lines.chrome.txt");
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let mut parts = line.split(" | ");
            let written = parts.next()?;
            let at_one = measured(parts.next()?)?;
            let at_two = measured(parts.next()?)?;
            let mut words = written.split_whitespace();
            let shrinks = words.next()? == "shrink";
            let width = words.next()?.parse().ok()?;
            let gap = words.next()?.parse().ok()?;
            let items = words
                .map(|item| {
                    let mut numbers = item.split('/').map(str::parse::<f64>);
                    Some(Item {
                        width: numbers.next()?.ok()?,
                        factor: numbers.next()?.ok()?,
                        padding: numbers.next()?.ok()?,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Line {
                shrinks,
                width,
                gap,
                items,
                at_one,
                at_two,
            })
        })
        .collect()
}

/// A length truncated onto the grid of `unit` per px (ADR 0054 § 1). The
/// lengths here are written with at most seven decimals, so a product a
/// double holds a hair under a whole number is that number.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a length of at most a few hundred px, in units"
)]
fn units(px: f64, unit: f64) -> i64 {
    (px * unit + 1e-9).floor() as i64
}

/// Rounded half away from zero.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a share no larger than the line"
)]
fn rounded(share: f64) -> i64 {
    share.round() as i64
}

#[expect(clippy::cast_precision_loss, reason = "a size in units, far below 2⁵³")]
fn real(units: i64) -> f64 {
    units as f64
}

/// Shares `free` among `items` from the last to the first: each takes the
/// space still unshared times its factor over the factors still unshared,
/// that ratio taken first, rounded half away from zero.
fn shared(free: i64, items: &[usize], factors: &[f64]) -> BTreeMap<usize, i64> {
    let mut left = free;
    let mut weight: f64 = items.iter().filter_map(|at| factors.get(*at)).sum();
    let mut shares = BTreeMap::new();
    for at in items.iter().rev() {
        let factor = factors.get(*at).copied().unwrap_or(0.0);
        let share = if weight > 0.0 {
            rounded(real(left) * (factor / weight))
        } else {
            0
        };
        shares.insert(*at, share);
        left -= share;
        weight -= factor;
    }
    shares
}

/// ADR 0054's rule for a flex line, as measured: each item's left and width
/// in units of `64 × scale` per px.
fn by_the_rule(line: &Line, scale: u8) -> Vec<(i64, i64)> {
    let unit = 64.0 * f64::from(scale);
    let count = line.items.len();
    let gap = units(line.gap, unit);
    let gaps = gap * i64::try_from(count.saturating_sub(1)).unwrap_or(0);
    let room = units(line.width, unit) - gaps;
    let inner: Vec<i64> = line.items.iter().map(|i| units(i.width, unit)).collect();
    let floor: Vec<i64> = line
        .items
        .iter()
        .map(|i| 2 * units(i.padding, unit))
        .collect();
    let base: Vec<i64> = inner.iter().zip(&floor).map(|(i, p)| i + p).collect();
    let initial = room - base.iter().sum::<i64>();
    // Only the factor the line uses flexes it: a shrinking line's grow
    // factors are zero, and a growing line's shrink factors are.
    let flexes = (initial < 0) == line.shrinks && initial != 0;
    let written: Vec<f64> = line
        .items
        .iter()
        .map(|i| if flexes { i.factor } else { 0.0 })
        .collect();
    // A shrink factor is scaled by the item's inner size on the grid.
    let factors: Vec<f64> = if line.shrinks {
        written
            .iter()
            .zip(&inner)
            .map(|(f, i)| f * real(*i))
            .collect()
    } else {
        written.clone()
    };
    let mut fixed: BTreeMap<usize, i64> = (0..count)
        .filter(|at| written.get(*at).is_none_or(|f| *f <= 0.0))
        .filter_map(|at| Some((at, *base.get(at)?)))
        .collect();
    loop {
        let open: Vec<usize> = (0..count).filter(|at| !fixed.contains_key(at)).collect();
        if open.is_empty() {
            break;
        }
        let taken: i64 =
            fixed.values().sum::<i64>() + open.iter().filter_map(|at| base.get(*at)).sum::<i64>();
        let mut free = room - taken;
        // Factors that sum below one flex only that part of the line's
        // first free space, truncated.
        let total: f64 = open.iter().filter_map(|at| written.get(*at)).sum();
        if total < 1.0 {
            let part = (real(initial) * total).trunc();
            if part.abs() < real(free).abs() {
                free = rounded(part);
            }
        }
        let shares = shared(free, &open, &factors);
        let targets: Vec<(usize, i64)> = open
            .iter()
            .map(|at| {
                let target =
                    base.get(*at).copied().unwrap_or(0) + shares.get(at).copied().unwrap_or(0);
                (*at, target)
            })
            .collect();
        let under: Vec<(usize, i64)> = targets
            .iter()
            .filter_map(|(at, target)| {
                let least = *floor.get(*at)?;
                (*target < least).then_some((*at, least))
            })
            .collect();
        if under.is_empty() {
            fixed.extend(targets);
            break;
        }
        fixed.extend(under);
    }
    let mut left = 0;
    fixed
        .values()
        .map(|width| {
            let placed = (left, *width);
            left += width + gap;
            placed
        })
        .collect()
}

#[test]
fn the_rule_gives_every_line_chrome_measured() {
    let lines = lines().expect("the reference reads");
    assert_eq!(lines.len(), 762);
    for (index, line) in lines.iter().enumerate() {
        for scale in [1, 2] {
            assert_eq!(
                by_the_rule(line, scale),
                line.chrome(scale),
                "line {index} at {scale}"
            );
        }
    }
}

#[test]
fn the_rule_gives_border_widths_its_line_at_two() {
    // `cases/border-widths`: five 24 px boxes in 176 px with 8 px gaps,
    // whose borders at two are 4.5, 0.5, 1.5, 2 and 4.5 px a side (whole
    // device pixels). They shrink 2 px between them. Chrome 154's widths at
    // two, from the case's `chrome-at-2.txt` in 1/128 px, are 4,173, 3,148,
    // 3,405, 3,533 and 4,173: the fourth item takes 52 units where the rest
    // take 51, because 103 units over two factors is a half. At one the
    // borders are 4, 1, 1, 2 and 4 px and the line fits exactly.
    let item = |border: f64| Item {
        width: 24.0,
        factor: 1.0,
        padding: border,
    };
    let line = |borders: [f64; 5]| Line {
        shrinks: true,
        width: 176.0,
        gap: 8.0,
        items: borders.map(item).into_iter().collect(),
        at_one: Vec::new(),
        at_two: Vec::new(),
    };
    assert_eq!(
        by_the_rule(&line([4.5, 0.5, 1.5, 2.0, 4.5]), 2),
        [
            (0, 4173),
            (5197, 3148),
            (9369, 3405),
            (13798, 3533),
            (18355, 4173)
        ]
    );
    assert_eq!(
        by_the_rule(&line([4.0, 1.0, 1.0, 2.0, 4.0]), 1),
        [
            (0, 2048),
            (2560, 1664),
            (4736, 1664),
            (6912, 1792),
            (9216, 2048)
        ]
    );
}

/// Every item of every line as this engine lays it out at `scale`: its left
/// relative to its line and its width, in units of `64 × scale` per px.
fn laid_out(lines: &[Line], scale: u8) -> Option<Vec<Vec<(f32, f32)>>> {
    let markup = |(index, line): (usize, &Line)| {
        let items = (0..line.items.len())
            .map(|at| format!("<div id=l{index}i{at}></div>"))
            .collect::<Vec<_>>()
            .concat();
        format!("<div id=l{index}>{items}</div>")
    };
    let html: String = core::iter::once(String::from("<body>"))
        .chain(lines.iter().enumerate().map(markup))
        .collect();
    let css: String = core::iter::once(String::from(
        "body { margin: 0 } div div { height: 2px; min-width: 0 }\n",
    ))
    .chain(
        lines
            .iter()
            .enumerate()
            .map(|(index, line)| line.css(index)),
    )
    .collect();
    let document = parse_document(&html);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(&css);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let screen = MediaContext::default().at_scale(f32::from(scale));
    let styles: StyleTree = resolve(&document, &sheets, &screen);
    let boxes = build(&document, &styles);
    let layout = compute(&boxes, &styles, Size::new(3000.0, 600.0), &NoText);
    let root = boxes.root()?;
    let by_node: BTreeMap<NodeId, BoxId> = core::iter::once(root)
        .chain(boxes.descendants(root))
        .filter_map(|id| Some((boxes.get(id)?.kind.node()?, id)))
        .collect();
    let ids: BTreeMap<String, NodeId> = document
        .descendants(document.root())
        .filter_map(|id| Some((document.element(id)?.attr("id")?.to_owned(), id)))
        .collect();
    let rect = |layout: &LayoutTree, id: String| layout.border_box(*by_node.get(ids.get(&id)?)?);
    let unit = 64.0 * f32::from(scale);
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let container = rect(&layout, format!("l{index}"))?;
            (0..line.items.len())
                .map(|at| {
                    let item = rect(&layout, format!("l{index}i{at}"))?;
                    Some((
                        (item.origin.x - container.origin.x) * unit,
                        item.size.width * unit,
                    ))
                })
                .collect()
        })
        .collect()
}

/// How far one of this engine's edges is from Chrome's, in units.
#[expect(clippy::cast_precision_loss, reason = "a size in units, far below 2²⁴")]
fn apart(ours: f32, chrome: i64) -> f32 {
    (ours - chrome as f32).abs()
}

/// Whether a line flexes by factors that sum below one with a gap between
/// its items: where `taffy` 0.14.0 takes the gaps away twice (queue item
/// 572), so its items flex by the whole free space where Chrome's flex by
/// that part of it.
fn under_one_with_a_gap(line: &Line) -> bool {
    let total: f64 = line.items.iter().map(|item| item.factor).sum();
    line.shrinks && total < 1.0 && line.gap > 0.0
}

#[test]
fn this_engine_is_two_units_from_chrome_at_most_until_taffy_has_the_hook() {
    // Today the shares are `taffy`'s, in floats, and each edge is floored
    // as the tree is read back (ADR 0054's addendum, § 1). That is within a
    // unit of the exact edge. Chrome's last-to-first rounding can itself be
    // a unit and a quarter from it — eight 10 px items shrunk by 3/64 px at
    // two put an edge at 3,839 where the exact one is 3,837.75 — so an edge
    // here is up to two units from Chrome's, never more, except where
    // `taffy` flexes the wrong amount (572). The counts of lines exact are
    // pinned: the build that takes ADR 0056's hook (502) raises them to
    // every line, and has to change this test to say so.
    let lines = lines().expect("the reference reads");
    for (scale, exact_today) in [(1, 224), (2, 232)] {
        let ours = laid_out(&lines, scale).expect("every item is laid out");
        let mut exact = 0;
        let mut wrong_amount = Vec::new();
        for (index, (line, placed)) in lines.iter().zip(&ours).enumerate() {
            let chrome = line.chrome(scale);
            assert_eq!(placed.len(), chrome.len(), "line {index} at {scale}");
            let mut furthest: f32 = 0.0;
            for (at, ((left, width), (chrome_left, chrome_width))) in
                placed.iter().zip(chrome).enumerate()
            {
                let right = left + width;
                assert!(
                    left.to_bits() == left.trunc().to_bits()
                        && right.to_bits() == right.trunc().to_bits(),
                    "line {index} item {at} at {scale} is on the grid: {left} {width}"
                );
                furthest = furthest
                    .max(apart(*left, *chrome_left))
                    .max(apart(right, chrome_left + chrome_width));
            }
            if under_one_with_a_gap(line) && furthest > 2.0 {
                wrong_amount.push(index);
            } else {
                assert!(
                    furthest <= 2.0,
                    "line {index} at {scale}: ours {placed:?}, Chrome's {chrome:?}"
                );
            }
            exact += usize::from(furthest.to_bits() == 0.0_f32.to_bits());
        }
        assert_eq!(exact, exact_today, "lines exact at {scale}");
        // Two lines measured shrink by factors summing to 0.8 and 0.3 with
        // gaps of 4 px and 0.5 px: Chrome's first item ends 6.55 px and
        // 0.23 px further on than this engine's.
        assert_eq!(wrong_amount, [501, 750], "at {scale}");
    }
}
