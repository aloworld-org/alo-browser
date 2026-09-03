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

/// The shorthands this expands, and the longhands each becomes.
///
/// Only the box shorthands, because they are the ones a user-agent sheet sets
/// and an author overrides. `border`, `background` and `font` have the same
/// shape of problem and are not here — the engine does not yet set any of them
/// in the user-agent sheet, so nothing collides, and expanding them correctly
/// means parsing values rather than splitting on spaces.
const SIDED: [(&str, [&str; 4]); 2] = [
    (
        "margin",
        ["margin-top", "margin-right", "margin-bottom", "margin-left"],
    ),
    (
        "padding",
        [
            "padding-top",
            "padding-right",
            "padding-bottom",
            "padding-left",
        ],
    ),
];

/// A shorthand's longhands, with the value each side takes.
///
/// Empty for anything that is not one of [`SIDED`], or for a value whose shape
/// this engine cannot split.
///
/// # `var()` is split too, and it took a wrong turn to learn why
///
/// The first version of this refused to expand a value containing `var()`, on
/// the reasoning that a custom property may hold several values and so which
/// side each part belongs to is not knowable until substitution. That is true
/// and it made things **worse**: an author's `padding: var(--a) var(--b)` was
/// then the only shorthand left unexpanded, so it lost to the user agent's
/// expanded `padding-left`, and every control on every alo screen lost its
/// padding. A picture showed it.
///
/// So a `var()` is one part like any other function, and splitting respects
/// parentheses. A custom property holding several values is still not handled —
/// but it is now a rare wrong answer rather than a common one.
fn expand(declaration: &Declaration) -> Vec<(String, String)> {
    let name = declaration.name.as_str();
    let Some((_, longhands)) = SIDED.iter().find(|(shorthand, _)| *shorthand == name) else {
        return Vec::new();
    };
    let value = declaration.value.trim();
    if value.is_empty() {
        return Vec::new();
    }
    let parts = top_level_parts(value);
    let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
    // One value is every side; two are vertical then horizontal; three add a
    // separate bottom; four are top, right, bottom, left. Anything else is not
    // a shorthand this engine can split, and is left whole to be refused where
    // it is read.
    let sides: [&str; 4] = match parts.as_slice() {
        [all] => [all, all, all, all],
        [vertical, horizontal] => [vertical, horizontal, vertical, horizontal],
        [top, horizontal, bottom] => [top, horizontal, bottom, horizontal],
        [top, right, bottom, left] => [top, right, bottom, left],
        _ => return Vec::new(),
    };
    longhands
        .iter()
        .zip(sides)
        .map(|(longhand, side)| ((*longhand).to_owned(), side.to_owned()))
        .collect()
}

/// Split on the spaces between values, not the ones inside them.
///
/// `1px calc(2px + 3px) var(--a, 4px 5px)` is three values, and a split on
/// whitespace makes it six — which would put `calc(2px` on one side and
/// `+ 3px)` on another.
fn top_level_parts(value: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for letter in value.chars() {
        match letter {
            '(' => {
                depth += 1;
                current.push(letter);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(letter);
            }
            letter if letter.is_ascii_whitespace() && depth == 0 => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            letter => current.push(letter),
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
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
        for (name, value) in expand(&declaration) {
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

#[cfg(test)]
mod expansion_tests {
    use super::*;

    fn sides(value: &str) -> Vec<(String, String)> {
        expand(&Declaration::new("margin", value, Importance::Normal))
    }

    /// One value is every side; two are vertical then horizontal; three add a
    /// separate bottom; four are top, right, bottom, left.
    #[test]
    fn a_shorthand_becomes_the_four_sides_it_means() {
        assert_eq!(
            sides("1px")
                .iter()
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>(),
            vec!["1px", "1px", "1px", "1px"]
        );
        assert_eq!(
            sides("1px 2px")
                .iter()
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>(),
            vec!["1px", "2px", "1px", "2px"]
        );
        assert_eq!(
            sides("1px 2px 3px")
                .iter()
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>(),
            vec!["1px", "2px", "3px", "2px"]
        );
        assert_eq!(
            sides("1px 2px 3px 4px")
                .iter()
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>(),
            vec!["1px", "2px", "3px", "4px"]
        );
    }

    #[test]
    fn the_longhands_are_named_for_their_sides() {
        assert_eq!(
            sides("0")
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            vec!["margin-top", "margin-right", "margin-bottom", "margin-left"]
        );
    }

    /// A `var()` is one part like any other function. The first version of
    /// this refused to expand them, which left an author's
    /// `padding: var(--a) var(--b)` as the only unexpanded shorthand — so it
    /// lost to the user agent's expanded `padding-left`, and every control on
    /// every alo screen lost its padding.
    #[test]
    fn a_shorthand_holding_a_variable_is_still_split_by_side() {
        assert_eq!(
            sides("var(--gap) 2px")
                .iter()
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>(),
            vec!["var(--gap)", "2px", "var(--gap)", "2px"]
        );
    }

    /// A split on whitespace would make three values into six, putting
    /// `calc(2px` on one side and `+ 3px)` on another.
    #[test]
    fn the_spaces_inside_a_value_are_not_the_spaces_between_them() {
        assert_eq!(
            sides("1px calc(2px + 3px) var(--a, 4px 5px)")
                .iter()
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>(),
            vec![
                "1px",
                "calc(2px + 3px)",
                "var(--a, 4px 5px)",
                "calc(2px + 3px)"
            ]
        );
    }

    #[test]
    fn a_shorthand_of_no_shape_this_engine_knows_is_left_whole() {
        assert!(sides("1px 2px 3px 4px 5px").is_empty());
        assert!(sides("").is_empty());
    }

    /// Only the box shorthands. `border` and `background` have the same shape
    /// of problem and are not expanded, because nothing sets them in the
    /// user-agent sheet and so nothing collides.
    #[test]
    fn only_the_shorthands_this_engine_expands_are_expanded() {
        assert!(
            expand(&Declaration::new(
                "border",
                "1px solid red",
                Importance::Normal
            ))
            .is_empty()
        );
        assert!(expand(&Declaration::new("color", "red", Importance::Normal)).is_empty());
    }
}
