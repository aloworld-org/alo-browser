/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a declaration is: a property name, a value, and whether it was marked
//! important.
//!
//! **The value is kept as written.** Stage 1 parses style sheets; deciding
//! what `padding: var(--gap) 2px` computes to is the cascade's job, and the
//! cascade is queue item 3. Keeping the source text rather than a half-parsed
//! shape is what makes that possible without re-parsing the sheet — and it is
//! what lets an unknown property be *kept and ignored* rather than dropped,
//! which `docs/features.md` asks for by name.

use core::fmt;

use crate::shorthand;

/// Whether a declaration was written with `!important`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Importance {
    /// The ordinary case.
    #[default]
    Normal,
    /// Marked `!important`. The cascade puts these in their own layer.
    Important,
}

impl Importance {
    /// Whether this is `!important`.
    pub fn is_important(self) -> bool {
        self == Importance::Important
    }
}

/// The name of a property.
///
/// Custom properties are a separate case rather than a special string, because
/// they behave differently in every way that matters: their names are
/// case-sensitive, their values are almost unconstrained, and they are what
/// alo's design system is built from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PropertyName {
    /// A custom property, such as `--surface`. The name includes its two
    /// leading dashes and keeps the case it was written in.
    Custom(Box<str>),
    /// An ordinary property, such as `padding-inline`, lowercased — ordinary
    /// property names are ASCII case-insensitive.
    Ident(Box<str>),
}

impl PropertyName {
    /// The name a declaration was written with.
    ///
    /// A name starting with `--` is a custom property and keeps its case;
    /// anything else is lowercased, because that is what CSS says identity is.
    pub fn parse(name: &str) -> Self {
        if name.starts_with("--") {
            PropertyName::Custom(name.into())
        } else {
            PropertyName::Ident(name.to_ascii_lowercase().into())
        }
    }

    /// The name, as text.
    pub fn as_str(&self) -> &str {
        match self {
            PropertyName::Custom(name) | PropertyName::Ident(name) => name,
        }
    }

    /// Whether this is a custom property.
    pub fn is_custom(&self) -> bool {
        matches!(self, PropertyName::Custom(_))
    }
}

impl fmt::Display for PropertyName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One declaration, as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    /// The property being set.
    pub name: PropertyName,
    /// The value, as written, with surrounding whitespace and any
    /// `!important` removed. Not interpreted: see the module documentation.
    pub value: String,
    /// Whether it was marked `!important`.
    pub importance: Importance,
}

impl Declaration {
    /// A declaration, from its parts.
    pub fn new(name: &str, value: &str, importance: Importance) -> Self {
        Self {
            name: PropertyName::parse(name),
            value: value.trim().to_owned(),
            importance,
        }
    }
}

impl fmt::Display for Declaration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name, self.value)?;
        if self.importance.is_important() {
            f.write_str(" !important")?;
        }
        Ok(())
    }
}

/// The declarations inside one pair of braces, in the order they were written.
///
/// Order is kept because the cascade needs it: two declarations of the same
/// property in the same block are resolved by which came last, and a block
/// that reordered them would resolve them wrongly.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeclarationBlock {
    declarations: Vec<Declaration>,
}

impl DeclarationBlock {
    /// An empty block.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a declaration to the end of the block.
    pub fn push(&mut self, declaration: Declaration) {
        // A shorthand becomes its longhands as well, here, at the moment it is
        // written down.
        //
        // # Why this has to happen before the cascade rather than after
        //
        // The cascade competes declarations **by property name**. So a
        // `padding-left` from one sheet and a `padding` from another never meet
        // — they are different keys — and whichever the reader happens to
        // consult first wins, regardless of origin or specificity.
        //
        // That was invisible for as long as the user-agent sheet set no box
        // longhands. The moment it did, an author writing `ul { padding: 0 }`
        // was silently overridden by the user agent, which is the cascade
        // upside down. Expanding here makes the two compete as the same
        // property, which is what they are.
        //
        // The longhands are inserted *at the shorthand's position*, so
        // `padding: 1em; padding-left: 0` still ends with a left of zero: the
        // explicit one is written after and the cascade's order rule decides.
        for (name, value) in shorthand::expand(&declaration) {
            self.declarations
                .push(Declaration::new(&name, &value, declaration.importance));
        }
        self.declarations.push(declaration);
    }

    /// The declarations, in the order they were written.
    pub fn iter(&self) -> core::slice::Iter<'_, Declaration> {
        self.declarations.iter()
    }

    /// How many declarations the block holds.
    pub fn len(&self) -> usize {
        self.declarations.len()
    }

    /// Whether the block holds nothing.
    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }

    /// The **last** declaration of this property, which is the one that wins
    /// within a block.
    pub fn get(&self, name: &PropertyName) -> Option<&Declaration> {
        self.declarations
            .iter()
            .rev()
            .find(|declaration| &declaration.name == name)
    }

    /// Every custom property the block sets, in order.
    ///
    /// alo's design system is custom properties throughout, so this is the
    /// question the cascade asks most.
    pub fn custom_properties(&self) -> impl Iterator<Item = &Declaration> {
        self.declarations
            .iter()
            .filter(|declaration| declaration.name.is_custom())
    }
}

impl<'a> IntoIterator for &'a DeclarationBlock {
    type Item = &'a Declaration;
    type IntoIter = core::slice::Iter<'a, Declaration>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl fmt::Display for DeclarationBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, declaration) in self.declarations.iter().enumerate() {
            if index > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{declaration};")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ordinary_property_name_is_lowercased_and_a_custom_one_is_not() {
        assert_eq!(
            PropertyName::parse("Padding-Inline"),
            PropertyName::Ident("padding-inline".into()),
        );
        assert_eq!(
            PropertyName::parse("--Surface"),
            PropertyName::Custom("--Surface".into()),
            "custom property names are case sensitive",
        );
        assert!(PropertyName::parse("--x").is_custom());
        assert!(!PropertyName::parse("color").is_custom());
        assert_eq!(PropertyName::parse("--x").as_str(), "--x");
    }

    #[test]
    fn a_declaration_keeps_its_value_as_written() {
        let declaration = Declaration::new("padding", "  var(--gap) 2px  ", Importance::Normal);
        assert_eq!(declaration.value, "var(--gap) 2px");
        assert_eq!(declaration.to_string(), "padding: var(--gap) 2px");

        let important = Declaration::new("color", "red", Importance::Important);
        assert!(important.importance.is_important());
        assert_eq!(important.to_string(), "color: red !important");
    }

    #[test]
    fn the_last_declaration_of_a_property_is_the_one_a_block_reports() {
        let mut block = DeclarationBlock::new();
        assert!(block.is_empty());
        block.push(Declaration::new("color", "red", Importance::Normal));
        block.push(Declaration::new("margin", "0", Importance::Normal));
        block.push(Declaration::new("color", "blue", Importance::Normal));

        assert_eq!(
            block.len(),
            7,
            "both colours are kept and the margin brought its four longhands"
        );
        assert_eq!(
            block
                .get(&PropertyName::parse("color"))
                .map(|declaration| &*declaration.value),
            Some("blue"),
        );
        assert_eq!(block.get(&PropertyName::parse("padding")), None);
    }

    #[test]
    fn a_block_can_name_the_custom_properties_it_sets() {
        let mut block = DeclarationBlock::new();
        block.push(Declaration::new("--surface", "#101014", Importance::Normal));
        block.push(Declaration::new("color", "var(--ink)", Importance::Normal));
        block.push(Declaration::new("--ink", "#f4f4f5", Importance::Normal));

        let names: Vec<_> = block
            .custom_properties()
            .map(|declaration| declaration.name.as_str())
            .collect();
        assert_eq!(names, vec!["--surface", "--ink"]);
    }

    #[test]
    fn a_block_writes_itself_back_out() {
        let mut block = DeclarationBlock::new();
        block.push(Declaration::new("color", "red", Importance::Normal));
        block.push(Declaration::new("--gap", "8px", Importance::Important));
        assert_eq!(block.to_string(), "color: red; --gap: 8px !important;");
        assert_eq!(DeclarationBlock::new().to_string(), "");
    }
}
