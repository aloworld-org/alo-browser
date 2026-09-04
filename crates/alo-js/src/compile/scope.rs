/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which name means which place, decided while compiling rather than while
//! running.
//!
//! There are two places a name can be, and this file is the only thing that
//! decides between them.
//!
//! - A **binding** of an environment: a function's parameters, its `var`s, its
//!   body-level `let` and `const`, the functions it declares — and everything a
//!   **block** inside it declares. These live in a cell in the heap, so a
//!   closure keeps them after the call or the block that made them has ended,
//!   and `hops` says how many environments out to walk.
//! - The **realm**, for a name no scope declares — a script's own top-level
//!   `let`, a property of the global object, or nothing at all. A script's top
//!   level is not a block, because a second `<script>` on the same page sees
//!   its bindings and no environment outlives its script.
//!
//! The compiler's frame slots are not here at all: nothing a script can name is
//! one, and the temporaries it takes for a `switch`'s discriminant or an
//! `a.b++` have no name to look up.
//!
//! # A scope has an environment when it declares something
//!
//! A function scope always has one, because a call always makes one — even for a
//! function of no parameters that declares nothing, so that `hops` means the
//! same thing whichever function it is counted from. A **block** scope has one
//! only when it declares a name, which is why [`Scopes::find`] asks the scope
//! rather than counting levels: a block with nothing in it adds no link to the
//! chain and so adds no hop.
//!
//! That is the whole of queue item 216. Until it, a block's names were frame
//! slots that died with the call, so a function reading one was refused by name
//! — the honest refusal for a shape whose right answer is *a fresh binding every
//! pass*, which a slot shared between passes cannot be.

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
    /// A binding of an environment, `hops` environments out.
    Binding {
        /// How many environments out.
        hops: u32,
        /// Which binding of it.
        slot: u32,
        /// What an assignment to it does.
        assignment: Assignment,
    },
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
    /// Whether it is a function's, which has an environment whether it declares
    /// anything or not because a call always makes one.
    function: bool,
    bindings: Vec<Binding>,
}

impl Scope {
    /// Whether this scope is a link in the environment chain, which is what a
    /// hop counts.
    fn has_environment(&self) -> bool {
        self.function || !self.bindings.is_empty()
    }
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

    /// How many names the innermost scope holds, which is the binding the next
    /// one takes in the environment that scope will make.
    ///
    /// [`None`] if there is no scope open — a script's top level, whose names
    /// are the realm's — or if it holds more names than an index can name.
    pub fn count_here(&self) -> Option<u32> {
        u32::try_from(self.open.last()?.bindings.len()).ok()
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
    /// `hops` counts the scopes passed on the way that **have an environment**,
    /// which is every function and every block that declares something. A block
    /// that declares nothing adds no link to the chain at run time, so counting
    /// it here would send every name past it one environment too far.
    pub fn find(&self, name: &str) -> Where {
        let mut hops = 0_u32;
        for scope in self.open.iter().rev() {
            if let Some(binding) = scope.bindings.iter().rev().find(|held| held.name == name) {
                return Where::Binding {
                    hops,
                    slot: binding.slot,
                    assignment: binding.assignment,
                };
            }
            if scope.has_environment() {
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
        assert_eq!(scopes.declare("a", 0, Assignment::Refused), Ok(()));
        assert_eq!(
            scopes.find("a"),
            Where::Binding {
                hops: 0,
                slot: 0,
                assignment: Assignment::Refused
            }
        );
        scopes.close();
        assert_eq!(
            scopes.find("a"),
            Where::Binding {
                hops: 0,
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
        assert_eq!(scopes.count_here(), Some(0));
        assert_eq!(scopes.declare("a", 0, Assignment::Allowed), Ok(()));
        assert_eq!(scopes.count_here(), Some(1));
        assert!(scopes.declare("a", 1, Assignment::Allowed).is_err());
        assert_eq!(scopes.declared_here("a"), Some(0));
        assert_eq!(scopes.declared_here("b"), None);
    }

    #[test]
    fn a_hop_is_a_scope_that_has_an_environment_and_an_empty_block_is_not_one() {
        let mut scopes = Scopes::new();
        scopes.open_function();
        assert_eq!(scopes.declare("outer", 3, Assignment::Allowed), Ok(()));
        scopes.open_block();
        assert_eq!(scopes.declare("blocked", 0, Assignment::Allowed), Ok(()));
        // A block that declares nothing makes no environment, so it is not a
        // hop — which is the whole reason a scope is asked rather than counted.
        scopes.open_block();
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
            scopes.find("blocked"),
            Where::Binding {
                hops: 1,
                slot: 0,
                assignment: Assignment::Allowed
            },
            "the block's own environment, past the empty block that has none"
        );
        assert_eq!(
            scopes.find("outer"),
            Where::Binding {
                hops: 2,
                slot: 3,
                assignment: Assignment::Allowed
            },
            "and the function's, one further out"
        );
        assert_eq!(scopes.find("nobody"), Where::Global);
    }
}
