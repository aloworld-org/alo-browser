/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which name means which place, decided while compiling rather than while
//! running.
//!
//! There are three places a name can be, and this file is the only thing that
//! decides between them.
//!
//! - A **binding** of a function's environment: its parameters, its `var`s, its
//!   body-level `let` and `const`, and the functions it declares. These live in
//!   a cell in the heap, so a closure keeps them after the call that made them
//!   has returned, and `hops` says how many function environments out to walk.
//! - A **frame slot** of the body being compiled: a block's own `let` and
//!   `const`, and the temporaries the compiler takes for itself. These die with
//!   the call.
//! - The **realm**, for a name no scope declares — a script's own top-level
//!   `let`, a property of the global object, or nothing at all. A script's top
//!   level is not a block, because a second `<script>` on the same page sees
//!   its bindings and neither a frame nor an environment outlives its script.
//!
//! # The one thing that is refused rather than compiled
//!
//! A function that reads a **block's** binding of an enclosing function is
//! [`Where::Captured`], and the compiler refuses it by name (queue item 216).
//! It is refused rather than compiled because the honest implementation is a
//! per-block environment, and the reason it has to be per-block is a loop: the
//! language gives `for (let i = …)` a fresh `i` every pass, so a closure made in
//! one pass and a closure made in the next must not see one binding. Sharing a
//! slot between passes is the wrong answer that reads like a right one, and this
//! engine's earlier note — *nothing can tell; a closure is what would* — stops
//! being true the moment there are closures. So the case that would show it is
//! the case that is refused.
//!
//! # A slot is never given back
//!
//! Two blocks that cannot both be live still get different slots. Reusing them
//! would save memory a script chose the size of, and the number of slots is
//! already bounded by how deep a tree the parser will build
//! ([`bounds::DEEPEST_EXPRESSION`](crate::bounds)) — so the saving is small and
//! the mistake it makes possible, a slot read by the block that took it next, is
//! the kind nothing finds for a year.

/// What an assignment to a name is allowed to do.
///
/// Three answers rather than two, and the third is not a nicety: assigning to a
/// **named function expression's own name** does nothing at all in sloppy code,
/// where assigning to a `const` is a `TypeError` in both modes. One enum, so
/// that the compiler cannot answer the second question with the first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assignment {
    /// `let`, `var`, a parameter: it takes the value.
    Allowed,
    /// `const`: a `TypeError`, in strict code and in sloppy code alike.
    Refused,
    /// A function expression's own name in sloppy code: the value is evaluated
    /// and then dropped, which is what the specification asks for.
    Ignored,
}

/// Where a name lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// A frame slot of the body being compiled.
    Local {
        /// Which slot.
        slot: u32,
        /// What an assignment to it does.
        assignment: Assignment,
    },
    /// A binding of an environment, `hops` function environments out.
    Binding {
        /// How many environments out.
        hops: u32,
        /// Which binding of it.
        slot: u32,
        /// What an assignment to it does.
        assignment: Assignment,
    },
    /// A block's binding of an **enclosing** function, which is queue item 216.
    Captured,
    /// Not declared by any scope: the realm answers for it at run time.
    Global,
}

/// One name a scope declares.
#[derive(Debug, Clone)]
struct Binding {
    name: String,
    slot: u32,
    assignment: Assignment,
}

/// One scope's worth of names.
#[derive(Debug)]
struct Scope {
    /// Whether it is a function's, whose names are environment bindings, rather
    /// than a block's, whose names are frame slots.
    function: bool,
    bindings: Vec<Binding>,
}

/// The scopes a compiler is inside, innermost last.
#[derive(Debug, Default)]
pub struct Scopes {
    open: Vec<Scope>,
}

/// A name declared twice in one scope.
///
/// `let a; let a;` is a program no engine runs, and refusing it is not
/// tidiness: the second declaration would either take a second place — so two
/// names that are one name in the source are two at run time — or reuse the
/// first, which puts a live binding back in its dead zone. Queue item 205 owns
/// the rest of the early errors that need a scope; this one is here because the
/// compiler cannot be correct without it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redeclared {
    /// The name.
    pub name: String,
}

impl Scopes {
    /// No scopes at all, which is a script's top level.
    pub fn new() -> Self {
        Self::default()
    }

    /// Go into a block.
    pub fn open_block(&mut self) {
        self.open.push(Scope {
            function: false,
            bindings: Vec::new(),
        });
    }

    /// Go into a function, whose names are bindings of an environment.
    pub fn open_function(&mut self) {
        self.open.push(Scope {
            function: true,
            bindings: Vec::new(),
        });
    }

    /// Come out of either.
    pub fn close(&mut self) {
        self.open.pop();
    }

    /// Whether the innermost scope already has this name, which is what lets a
    /// function's `var` find the parameter of the same name rather than taking
    /// a second binding for it.
    pub fn declared_here(&self, name: &str) -> Option<u32> {
        let scope = self.open.last()?;
        scope
            .bindings
            .iter()
            .find(|binding| binding.name == name)
            .map(|binding| binding.slot)
    }

    /// Declare a name in the innermost scope.
    ///
    /// # Errors
    ///
    /// [`Redeclared`] if that scope already has it.
    pub fn declare(
        &mut self,
        name: &str,
        slot: u32,
        assignment: Assignment,
    ) -> Result<(), Redeclared> {
        let Some(scope) = self.open.last_mut() else {
            return Ok(());
        };
        if scope.bindings.iter().any(|binding| binding.name == name) {
            return Err(Redeclared {
                name: name.to_owned(),
            });
        }
        scope.bindings.push(Binding {
            name: name.to_owned(),
            slot,
            assignment,
        });
        Ok(())
    }

    /// Where a name is, looking outwards from the innermost scope.
    ///
    /// `hops` counts the **function** scopes passed on the way, because that is
    /// what the environment chain has a link for. A block adds no link, which is
    /// also why a block's binding cannot be reached from inside a function:
    /// there is nothing to walk.
    pub fn find(&self, name: &str) -> Where {
        let mut hops = 0_u32;
        for scope in self.open.iter().rev() {
            for binding in scope.bindings.iter().rev() {
                if binding.name != name {
                    continue;
                }
                if scope.function {
                    return Where::Binding {
                        hops,
                        slot: binding.slot,
                        assignment: binding.assignment,
                    };
                }
                return if hops == 0 {
                    Where::Local {
                        slot: binding.slot,
                        assignment: binding.assignment,
                    }
                } else {
                    Where::Captured
                };
            }
            if scope.function {
                hops = hops.saturating_add(1);
            }
        }
        Where::Global
    }
}

#[cfg(test)]
mod tests {
    use super::{Assignment, Scopes, Where};

    #[test]
    fn an_inner_block_shadows_an_outer_one_and_gives_it_back() {
        let mut scopes = Scopes::new();
        scopes.open_block();
        assert_eq!(scopes.declare("a", 0, Assignment::Allowed), Ok(()));
        scopes.open_block();
        assert_eq!(scopes.declare("a", 1, Assignment::Refused), Ok(()));
        assert_eq!(
            scopes.find("a"),
            Where::Local {
                slot: 1,
                assignment: Assignment::Refused
            }
        );
        scopes.close();
        assert_eq!(
            scopes.find("a"),
            Where::Local {
                slot: 0,
                assignment: Assignment::Allowed
            }
        );
        scopes.close();
        assert_eq!(scopes.find("a"), Where::Global);
    }

    #[test]
    fn one_scope_may_not_declare_a_name_twice() {
        let mut scopes = Scopes::new();
        scopes.open_block();
        assert_eq!(scopes.declare("a", 0, Assignment::Allowed), Ok(()));
        assert!(scopes.declare("a", 1, Assignment::Allowed).is_err());
        assert_eq!(scopes.declared_here("a"), Some(0));
        assert_eq!(scopes.declared_here("b"), None);
    }

    #[test]
    fn hops_count_functions_and_a_blocks_binding_cannot_be_reached_across_one() {
        let mut scopes = Scopes::new();
        scopes.open_function();
        assert_eq!(scopes.declare("outer", 3, Assignment::Allowed), Ok(()));
        scopes.open_block();
        assert_eq!(scopes.declare("blocked", 1, Assignment::Allowed), Ok(()));
        scopes.open_function();
        assert_eq!(scopes.declare("mine", 0, Assignment::Allowed), Ok(()));

        assert_eq!(
            scopes.find("mine"),
            Where::Binding {
                hops: 0,
                slot: 0,
                assignment: Assignment::Allowed
            }
        );
        assert_eq!(
            scopes.find("outer"),
            Where::Binding {
                hops: 1,
                slot: 3,
                assignment: Assignment::Allowed
            },
            "one environment out, past the block that has none"
        );
        assert_eq!(
            scopes.find("blocked"),
            Where::Captured,
            "a block's binding has no link to walk — queue item 216"
        );
    }
}
