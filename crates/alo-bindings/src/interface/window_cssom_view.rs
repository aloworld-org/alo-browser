/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! CSSOM View's `partial interface Window` (ADR 0038, queue item 366): what
//! a page reads of its viewport.
//!
//! - `innerWidth` and `innerHeight` — `[Replaceable] readonly attribute
//!   long`: the size the page is laid out at, its [`View`]'s viewport, asked
//!   at every read so that a read after a `Resize` answers the new size
//!   (§ 2). Rounded to the nearest integer, a half up, and clamped to
//!   `0 ..= 2³¹ − 1`; a size that is not finite answers `0`. No scrollbar is
//!   subtracted, since none is drawn.
//! - `scrollX` and `scrollY` — `[Replaceable] readonly attribute double`:
//!   where the viewport is scrolled to, which the renderer holds (§ 3).
//!   `pageXOffset` and `pageYOffset` are the specification's other names for
//!   the same two getters, and so are built with them. A position that is
//!   not finite — which a `double` cannot hold — answers `0`.
//!
//! `Window` is `[Global]`, so each is an accessor **on the one instance**,
//! enumerable and configurable, beside `self`. Each is `[Replaceable]`: its
//! setter puts an own data property of its name in its place, as `self`'s
//! does ([`super::window::replace`]), so a page's `innerWidth = 5` reads 5
//! afterwards and asks nothing.
//!
//! A page shown no view refuses each by name, a `TypeError`, rather than
//! make up a size (§ 5, ADR 0013 § 3).
//!
//! # Not here
//!
//! Everything beyond the window stays absent (§ 1): `screen`,
//! `outerWidth`, `outerHeight`, `screenX`, `screenY`, `screenLeft`,
//! `screenTop` and `devicePixelRatio`. Each is opened by a frozen page that
//! fails without it, and answered then under ADR 0030's rule, never with the
//! machine's value.
//!
//! [`View`]: crate::view::View

use alo_js::Escape;
use alo_js::abrupt::Internal;
use alo_js::convert;
use alo_js::heap::Ref;
use alo_js::object::native::{Answer, Call};
use alo_js::object::{Objects, Value};

use super::window::{replace, this};
use crate::define;
use crate::view::View;
use crate::window::Window;

/// What a member reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reads {
    Width,
    Height,
    X,
    Y,
}

/// Every member, by name, in the order the specification lists them; a
/// native is made around its index here.
const MEMBERS: [(&str, Reads); 6] = [
    ("innerWidth", Reads::Width),
    ("innerHeight", Reads::Height),
    ("scrollX", Reads::X),
    ("pageXOffset", Reads::X),
    ("scrollY", Reads::Y),
    ("pageYOffset", Reads::Y),
];

/// Every member, on the window `global`.
///
/// **A safepoint.** `global` is the realm's global object, which the realm
/// roots.
///
/// # Errors
///
/// [`Escape::Full`] when the heap cannot hold them, and a fault for a
/// reference this engine has lost.
pub(super) fn members(
    objects: &mut Objects,
    global: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    for (at, (name, _)) in (0_u32..).zip(MEMBERS) {
        define::attribute_holding(
            objects,
            global,
            function_prototype,
            name,
            (get, set),
            Value::Number(f64::from(at)),
        )?;
    }
    Ok(())
}

/// The member a native was made around.
fn member(call: &Call<'_>) -> Result<(&'static str, Reads), Escape> {
    let Value::Number(at) = call.held() else {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    };
    usize::try_from(convert::to_uint32(at))
        .ok()
        .and_then(|at| MEMBERS.get(at).copied())
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))
}

/// A member's getter: its view asked, now.
fn get(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let (name, reads) = member(call)?;
    let window = this(call, name)?;
    let view = call
        .seen()
        .embedded::<Window>(window)
        .and_then(Window::view)
        .ok_or_else(|| {
            Escape::type_error(
                format!("this page was given no view, so it cannot say its '{name}'"),
                call.at(),
            )
        })?;
    Ok(Answer::Value(Value::Number(read(view.as_ref(), reads))))
}

/// A member's setter: `[Replaceable]`.
fn set(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let (name, _) = member(call)?;
    replace(call, name)
}

/// What `view` answers for `reads`, as the member's type holds it.
fn read(view: &dyn View, reads: Reads) -> f64 {
    match reads {
        Reads::Width => long(view.viewport().width),
        Reads::Height => long(view.viewport().height),
        Reads::X => double(view.scrolled().x),
        Reads::Y => double(view.scrolled().y),
    }
}

/// A size, as a Web IDL `long` holds it here (ADR 0038 § 2): the nearest
/// integer, a half rounded up; clamped to `0 ..= 2³¹ − 1`; and `0` for a
/// size that is not finite. Never `-0`. An element's `scrollWidth` and
/// `scrollHeight` are held the same way ([`super::element_cssom_view`]).
pub(super) fn long(size: f64) -> f64 {
    if !size.is_finite() {
        return 0.0;
    }
    let floor = size.floor();
    let nearest = if size - floor >= 0.5 {
        floor + 1.0
    } else {
        floor
    };
    // `+ 0.0` makes a `-0` the `0` a `long` is.
    nearest.clamp(0.0, f64::from(i32::MAX)) + 0.0
}

/// A position, as a Web IDL `double` holds it: as it is, or `0` when it is
/// not finite, which a `double` cannot be.
fn double(position: f64) -> f64 {
    if position.is_finite() { position } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::{MEMBERS, Reads, double, long};

    #[test]
    fn a_size_is_rounded_to_the_nearest_whole_pixel_a_half_up() {
        assert_eq!(long(800.0).to_bits(), 800.0_f64.to_bits());
        assert_eq!(long(800.5).to_bits(), 801.0_f64.to_bits());
        assert_eq!(long(800.499_999).to_bits(), 800.0_f64.to_bits());
        assert_eq!(long(599.6).to_bits(), 600.0_f64.to_bits());
        assert_eq!(long(0.5).to_bits(), 1.0_f64.to_bits());
        // The largest `f64` below a half: adding a half would round up.
        assert_eq!(long(0.499_999_999_999_999_94).to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn a_negative_a_huge_and_a_size_that_is_not_finite_are_clamped() {
        for negative in [-1.0, -0.4, -0.6, -0.0, -1.0e300] {
            assert_eq!(long(negative).to_bits(), 0.0_f64.to_bits(), "{negative}");
        }
        assert_eq!(long(1.0e300).to_bits(), 2_147_483_647.0_f64.to_bits());
        assert_eq!(
            long(2_147_483_647.4).to_bits(),
            2_147_483_647.0_f64.to_bits()
        );
        for odd in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            assert_eq!(long(odd).to_bits(), 0.0_f64.to_bits(), "{odd}");
        }
    }

    #[test]
    fn a_position_is_as_it_is_unless_it_is_not_finite() {
        assert_eq!(double(12.25).to_bits(), 12.25_f64.to_bits());
        assert_eq!(double(0.0).to_bits(), 0.0_f64.to_bits());
        for odd in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            assert_eq!(double(odd).to_bits(), 0.0_f64.to_bits(), "{odd}");
        }
    }

    #[test]
    fn the_page_offsets_are_the_scroll_positions_by_other_names() {
        let reads = |name: &str| {
            MEMBERS
                .iter()
                .find(|(member, _)| *member == name)
                .map(|(_, reads)| *reads)
        };
        assert_eq!(reads("pageXOffset"), reads("scrollX"));
        assert_eq!(reads("pageYOffset"), reads("scrollY"));
        assert_eq!(reads("innerWidth"), Some(Reads::Width));
        assert_eq!(reads("innerHeight"), Some(Reads::Height));
    }
}
