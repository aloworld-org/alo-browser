/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Validate the supported function headers before assigning their bindings.
//!
//! The parser reads parameters before a body's directive prologue. The
//! compiler sees the final strictness, so a local `"use strict"` applies to
//! the header too. This is an early error, even if the function is never called.
//! Unsupported parameter forms remain queue item 213; their early errors are
//! queue item 222. No second scope table is built here.

use crate::ast::{Function, FunctionKind, Pattern};
use crate::word::{Keyword, Status, keyword};

use super::{Refusal, What};

/// The names of a function's parameters, refusing every form that is not one.
///
/// Four refusals and one item: a default, a `...rest` and a destructuring
/// pattern each need a value taken apart before the body starts, and a repeated
/// name in an ordinary sloppy function needs semantics not yet built. Those
/// unsupported forms answer with item 213's [`What`]. Duplicates forbidden by
/// strict code or an arrow are early errors instead.
pub(super) fn parameter_names(function: &Function) -> Result<Vec<String>, Refusal> {
    let at = function.start;
    if function.kind != FunctionKind::Plain {
        // An `async` or a generator is a function whose frame is suspended and
        // resumed, which is queue item 75 rather than a shape of parameter.
        return Err(Refusal::NotBuiltYet {
            what: What::ASuspension,
            at,
        });
    }
    if function.rest.is_some() {
        return Err(Refusal::NotBuiltYet {
            what: What::AParameterForm,
            at,
        });
    }
    if function.strict {
        if let Some(name) = &function.name {
            strict_name(name, at)?;
        }
    }
    let mut names: Vec<String> = Vec::new();
    for element in &function.parameters {
        if element.default.is_some() {
            return Err(Refusal::NotBuiltYet {
                what: What::AParameterForm,
                at,
            });
        }
        let Pattern::Name(name) = &element.pattern else {
            return Err(Refusal::NotBuiltYet {
                what: What::AParameterForm,
                at,
            });
        };
        if function.strict {
            strict_name(name, at)?;
        }
        if names.contains(name) && (function.strict || function.is_arrow) {
            return Err(Refusal::NotAProgram {
                why: format!("'{name}' is a parameter of this function twice"),
                at,
            });
        }
        if names.contains(name) {
            return Err(Refusal::NotBuiltYet {
                what: What::AParameterForm,
                at,
            });
        }
        names.push(name.clone());
    }
    Ok(names)
}

/// Names have already been decoded, so escapes cannot hide a reserved name.
fn strict_name(name: &str, at: usize) -> Result<(), Refusal> {
    let reserved = keyword(name).is_some_and(|word| {
        word.status() == Status::ReservedInStrictCode || word == Keyword::Yield
    });
    if reserved || matches!(name, "eval" | "arguments") {
        return Err(Refusal::NotAProgram {
            why: format!("'{name}' cannot bind a name in a strict function header"),
            at,
        });
    }
    Ok(())
}
