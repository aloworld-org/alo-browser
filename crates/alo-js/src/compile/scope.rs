/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which name means which slot, decided while compiling rather than while
//! running.
//!
//! A block's `let` and `const` bindings become **frame slots**, and a name in
//! the source becomes the index of one. That is the whole of what a scope is
//! here, and doing it at compile time is what makes reading a local variable one
//! instruction with no lookup in it.
//!
//! # Why the top level is not a block
//!
//! A script's own top-level `let` and `const` belong to the **realm** rather
//! than to the frame: a second `<script>` on the same page sees them, and a
//! frame ends with its script. So [`Blocks::find`] answers [`Where::Global`] for
//! a name no block declares, and the realm decides at run time whether that is a
//! lexical binding, a property of the global object, or nothing at all
//! ([`realm`](crate::realm)).
//!
//! # A slot is never given back
//!
//! Two blocks that cannot both be live still get different slots. Reusing them
//! would save memory a script chose the size of, and the number of slots is
//! already bounded by how deep a tree the parser will build
//! ([`bounds::DEEPEST_EXPRESSION`](crate::bounds)) — so the saving is small and
//! the mistake it makes possible, a slot read by the block that took it next, is
//! the kind nothing finds for a year.

/// Where a name lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// A frame slot, and whether it may be assigned to.
    Local {
        /// Which slot.
        slot: u32,
        /// `let` may be assigned to again; `const` may not.
        mutable: bool,
    },
    /// Not a frame slot: the realm answers for it at run time.
    Global,
}

/// One name a block declares.
#[derive(Debug, Clone)]
struct Binding {
    name: String,
    slot: u32,
    mutable: bool,
}

/// One block's worth of bindings.
#[derive(Debug, Default)]
struct Block {
    bindings: Vec<Binding>,
}

/// The blocks a compiler is inside, innermost last.
#[derive(Debug, Default)]
pub struct Blocks {
    open: Vec<Block>,
}

/// A name declared twice in one block.
///
/// `let a; let a;` is a program no engine runs, and refusing it is not
/// tidiness: the second declaration would either take a second slot — so two
/// names that are one name in the source are two at run time — or reuse the
/// first, which puts a live binding back in its dead zone. Queue item 205 owns
/// the rest of the early errors that need a scope; this one is here because the
/// compiler cannot be correct without it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redeclared {
    /// The name.
    pub name: String,
}

impl Blocks {
    /// No blocks at all, which is a script's top level.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether anything is open, which is how a declaration knows whether it is
    /// the realm's or a frame's.
    pub fn inside_a_block(&self) -> bool {
        !self.open.is_empty()
    }

    /// Go into a block.
    pub fn open(&mut self) {
        self.open.push(Block::default());
    }

    /// Come out of one.
    pub fn close(&mut self) {
        self.open.pop();
    }

    /// Declare a name in the innermost block.
    ///
    /// # Errors
    ///
    /// [`Redeclared`] if that block already has it.
    pub fn declare(&mut self, name: &str, slot: u32, mutable: bool) -> Result<(), Redeclared> {
        let Some(block) = self.open.last_mut() else {
            return Ok(());
        };
        if block.bindings.iter().any(|binding| binding.name == name) {
            return Err(Redeclared {
                name: name.to_owned(),
            });
        }
        block.bindings.push(Binding {
            name: name.to_owned(),
            slot,
            mutable,
        });
        Ok(())
    }

    /// Where a name is, looking outwards from the innermost block.
    pub fn find(&self, name: &str) -> Where {
        for block in self.open.iter().rev() {
            for binding in block.bindings.iter().rev() {
                if binding.name == name {
                    return Where::Local {
                        slot: binding.slot,
                        mutable: binding.mutable,
                    };
                }
            }
        }
        Where::Global
    }
}

#[cfg(test)]
mod tests {
    use super::{Blocks, Where};

    #[test]
    fn an_inner_block_shadows_an_outer_one_and_gives_it_back() {
        let mut blocks = Blocks::new();
        blocks.open();
        assert_eq!(blocks.declare("a", 0, true), Ok(()));
        blocks.open();
        assert_eq!(blocks.declare("a", 1, false), Ok(()));
        assert_eq!(
            blocks.find("a"),
            Where::Local {
                slot: 1,
                mutable: false
            }
        );
        blocks.close();
        assert_eq!(
            blocks.find("a"),
            Where::Local {
                slot: 0,
                mutable: true
            }
        );
        blocks.close();
        assert_eq!(blocks.find("a"), Where::Global);
    }

    #[test]
    fn one_block_may_not_declare_a_name_twice() {
        let mut blocks = Blocks::new();
        blocks.open();
        assert_eq!(blocks.declare("a", 0, true), Ok(()));
        assert!(blocks.declare("a", 1, true).is_err());
    }
}
