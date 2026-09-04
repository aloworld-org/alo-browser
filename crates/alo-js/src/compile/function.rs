/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! One function into a chunk of its own.
//!
//! A function is a body, so it compiles the way a script's body does — and the
//! three things that make it different are all decided here rather than
//! anywhere downstream.
//!
//! # Its names are bindings, not slots
//!
//! A function's parameters, its `var`s, its body-level `let` and `const` and the
//! functions it declares all live in the **environment** a call is given, which
//! is a cell in the heap. That is what makes a closure possible: the environment
//! outlives the call, kept alive by the functions that closed over it.
//!
//! A block *inside* the function still uses frame slots, which die with the
//! call — see [`scope`](super::scope) for why the two are apart and what is
//! refused because of it.
//!
//! # What a call does before the first instruction
//!
//! Three things this file cannot emit an instruction for, because they happen
//! before there is a program counter: the arguments are written into bindings
//! `0..parameters`, a missing one as `undefined`; a named function expression's
//! own name is written into [`Chunk::own_slot`]; and every binding starts in its
//! dead zone. Everything else *is* an instruction, at the top of the chunk: a
//! `var` is given `undefined`, and a declared function is made and given its
//! name — which is the whole of what "hoisted" means, and why a function can be
//! called above the line that declares it and a `let` cannot be read there.
//!
//! # And it is compiled inside the scopes it was written in
//!
//! The compiler suspends the body it was in and keeps the **scopes**, because
//! those are exactly what the function closes over. A function compiled in a
//! scope stack of its own would resolve every outer name to the realm, which is
//! a closure that has quietly stopped being one.

use crate::ast::{Body, Function, FunctionKind, Pattern};
use crate::code::{Chunk, Op};

use super::hoist;
use super::scope::Assignment;
use super::{Compiler, Refusal, Suspended, What};

/// Whose the function's name is.
///
/// A declaration's name belongs to the scope around it — `function f() {}`
/// declares `f` there, and `f = 1` inside the body assigns *that* one. A
/// function **expression**'s name belongs to nobody else, so it is a binding
/// only the body can see, and the language makes it unassignable: a
/// `TypeError` in strict code and, for one of the language's older reasons,
/// silence in sloppy code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Naming {
    /// A declaration or a method: the scope around it holds the name.
    Outside,
    /// A function expression: it holds its own.
    Itself,
}

impl Compiler {
    /// Compile a function, answering which chunk of the unit it became.
    pub(super) fn function_chunk(
        &mut self,
        function: &Function,
        naming: Naming,
    ) -> Result<u32, Refusal> {
        let at = function.start;
        let names = parameter_names(function)?;

        let suspended = Suspended {
            chunk: std::mem::replace(&mut self.chunk, Chunk::new(function.strict)),
            enclosing: std::mem::take(&mut self.enclosing),
            chains: std::mem::take(&mut self.chains),
            script: std::mem::replace(&mut self.script, false),
        };
        self.suspended.push(suspended);
        self.scopes.open_function();

        let outcome = self.function_inside(function, &names, naming);

        self.scopes.close();
        let Some(saved) = self.suspended.pop() else {
            return Err(super::lost(at));
        };
        self.enclosing = saved.enclosing;
        self.chains = saved.chains;
        self.script = saved.script;
        let finished = std::mem::replace(&mut self.chunk, saved.chunk);
        outcome?;

        self.unit.add(finished).ok_or_else(|| Refusal::NotAProgram {
            why: "this program holds more functions than an index can name".to_owned(),
            at,
        })
    }

    /// The function's own body, with its scope already open and its chunk
    /// already installed.
    fn function_inside(
        &mut self,
        function: &Function,
        names: &[String],
        naming: Naming,
    ) -> Result<(), Refusal> {
        let at = function.start;
        if function.is_arrow {
            self.chunk.make_arrow();
        }

        // The parameters are bindings `0..parameters`, in the order they were
        // written, because that is the order a call fills them in.
        for name in names {
            let slot = self.binding(at)?;
            if self
                .scopes
                .declare(name, slot, Assignment::Allowed)
                .is_err()
            {
                return Err(Refusal::NotAProgram {
                    why: format!("'{name}' is a parameter of this function twice"),
                    at,
                });
            }
        }
        self.chunk.count_parameters(names.len());

        if naming == Naming::Itself {
            if let Some(own) = &function.name {
                let slot = self.binding(at)?;
                let assignment = if function.strict {
                    Assignment::Refused
                } else {
                    Assignment::Ignored
                };
                if self.scopes.declare(own, slot, assignment).is_err() {
                    return Err(Refusal::NotAProgram {
                        why: format!("'{own}' is this function's own name and a parameter of it"),
                        at,
                    });
                }
                let text = self.text(own)?;
                self.chunk.name_itself(text, Some(slot));
            }
        }

        match &function.body {
            // `() => a`, which returns without saying so and declares nothing.
            Body::Expression(value) => {
                self.expression(value)?;
                self.chunk.emit(Op::Return, at);
            }
            Body::Block(body) => self.function_body(body, at)?,
        }

        // A function that runs off the end answers `undefined`. Emitted always,
        // because a body that ends in a `return` costs two dead instructions and
        // a body that does not would otherwise walk off its own chunk.
        self.chunk.emit(Op::Undefined, at);
        self.chunk.emit(Op::Return, at);
        Ok(())
    }

    /// A `{ … }` body: what it declares, given its bindings, and then what it
    /// does.
    fn function_body(&mut self, body: &[crate::ast::Statement], at: usize) -> Result<(), Refusal> {
        // A `var` of the same name as a parameter is that parameter, which is
        // why this asks before it takes a binding: `function f(a) { var a; }`
        // is one name and one place.
        let mut names = Vec::new();
        hoist::vars(body, &mut names);
        let mut vars = Vec::new();
        for name in &names {
            if self.scopes.declared_here(name).is_some() {
                continue;
            }
            let slot = self.binding(at)?;
            let _ = self.scopes.declare(name, slot, Assignment::Allowed);
            vars.push(slot);
        }

        let declared = hoist::functions(body);
        let mut functions = Vec::new();
        for one in &declared {
            let Some(name) = &one.name else {
                continue;
            };
            let slot = if let Some(slot) = self.scopes.declared_here(name) {
                slot
            } else {
                let slot = self.binding(one.start)?;
                let _ = self.scopes.declare(name, slot, Assignment::Allowed);
                slot
            };
            functions.push((slot, *one));
        }

        for one in hoist::lexical(body) {
            if self.scopes.declared_here(&one.name).is_some() {
                return Err(Refusal::NotAProgram {
                    why: format!("'{}' is declared twice in this function's body", one.name),
                    at: one.at,
                });
            }
            let slot = self.binding(one.at)?;
            let assignment = if one.mutable {
                Assignment::Allowed
            } else {
                Assignment::Refused
            };
            let _ = self.scopes.declare(&one.name, slot, assignment);
        }

        // A `var` is readable before its line and is `undefined` there; a `let`
        // is in its dead zone, which is what an environment's bindings already
        // are.
        for slot in vars {
            self.chunk.emit(Op::Undefined, at);
            self.chunk.emit(Op::InitializeBinding { hops: 0, slot }, at);
        }
        // The functions come after, so that `var f; function f() {}` is the
        // function rather than `undefined`.
        for (slot, one) in functions {
            let which = self.function_chunk(one, Naming::Outside)?;
            self.chunk.emit(Op::Closure(which), one.start);
            self.chunk
                .emit(Op::InitializeBinding { hops: 0, slot }, one.start);
        }

        self.statements(body)
    }
}

/// The names of a function's parameters, refusing every form that is not one.
///
/// Four refusals and one item: a default, a `...rest` and a destructuring
/// pattern each need a value taken apart before the body starts, and a repeated
/// name needs the parameter scope the specification gives a function that has
/// one. Queue item 213 is all four, which is why they answer with one
/// [`What`].
fn parameter_names(function: &Function) -> Result<Vec<String>, Refusal> {
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
