//! What a box *is*.
//!
//! ADR 0002: **roles are declared, not inferred.** An agent reads "invoice
//! list, twelve rows, row three selected" and acts on it, so the answer to
//! "what is this" has to come from something the author wrote, never from what
//! the box looks like. Guessing a role from appearance is what screen-scraping
//! already does badly, and owning the engine is precisely what lets us not.
//!
//! There are two declared sources, in order:
//!
//! 1. **The `role` attribute**, which is the author saying it outright. A role
//!    this engine does not know is kept as written rather than dropped — the
//!    author still declared something, and a later stage should not have to
//!    re-read the document to find out what.
//! 2. **The element**, whose role HTML defines. `<nav>` is a navigation
//!    landmark because the standard says so, not because of where it sits or
//!    how it is styled. That is reading a declaration, not inferring one.
//!
//! What is deliberately absent is a third source. There is no "it has a border
//! and some rows, so it is probably a table".

use alo_dom::{Document, Element, NodeId};
use core::fmt;

/// What a box is, as an agent or a screen reader would name it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    /// A box with no role of its own — a `<div>` or a `<span>`. It still holds
    /// what is inside it; it just is not anything itself.
    Generic,
    /// `role="presentation"` or `role="none"`: the author saying this box
    /// carries no meaning and should be read through.
    Presentational,
    /// A role this engine has a name for.
    Known(KnownRole),
    /// A role the author declared that this engine does not know, kept exactly
    /// as written.
    Declared(Box<str>),
}

/// The roles this engine knows by name.
///
/// The list is what a modern interface is built from — landmarks, structure,
/// and the widgets a person operates — rather than the whole of ARIA. A role
/// outside it is still carried, as [`Role::Declared`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KnownRole {
    /// The document itself.
    Document,
    /// A page's own header.
    Banner,
    /// A set of navigation links.
    Navigation,
    /// The main content.
    Main,
    /// Content related to the main content.
    Complementary,
    /// A page's own footer.
    ContentInfo,
    /// A named area of the page.
    Region,
    /// A form.
    Form,
    /// A search area.
    Search,
    /// A self-contained piece of content.
    Article,
    /// A heading, with its level in [`crate::state::States`].
    Heading,
    /// A paragraph.
    Paragraph,
    /// A list.
    List,
    /// An item of a list.
    ListItem,
    /// A grouping with no stronger meaning.
    Group,
    /// A thematic break.
    Separator,
    /// A quotation.
    Blockquote,
    /// A figure.
    Figure,
    /// A picture.
    Image,
    /// A button.
    Button,
    /// A hyperlink.
    Link,
    /// A field a person types into.
    TextBox,
    /// A field a person types a search into.
    SearchBox,
    /// A checkbox.
    CheckBox,
    /// A radio button.
    Radio,
    /// A group of radio buttons.
    RadioGroup,
    /// A control that switches between two states.
    Switch,
    /// A control that combines a field and a list.
    ComboBox,
    /// A list a person chooses from.
    ListBox,
    /// An option in such a list.
    Option,
    /// A control set by dragging along a range.
    Slider,
    /// A control set by stepping through numbers.
    SpinButton,
    /// A bar showing how far something has got.
    ProgressBar,
    /// A gauge.
    Meter,
    /// A dialog.
    Dialog,
    /// A status message.
    Status,
    /// A grid of rows and cells — a table a person operates.
    Grid,
    /// A row of a grid or table.
    Row,
    /// A cell of a row.
    Cell,
    /// A group of rows.
    RowGroup,
    /// A cell that heads its column.
    ColumnHeader,
    /// A cell that heads its row.
    RowHeader,
    /// A table.
    Table,
    /// A tab.
    Tab,
    /// The strip a set of tabs sits in.
    TabList,
    /// The panel a tab reveals.
    TabPanel,
    /// A menu.
    Menu,
    /// An item of a menu.
    MenuItem,
    /// A summary that discloses something.
    Summary,
    /// A disclosure and its content.
    Details,
}

impl KnownRole {
    /// The ARIA name for this role.
    pub fn as_str(self) -> &'static str {
        match self {
            KnownRole::Document => "document",
            KnownRole::Banner => "banner",
            KnownRole::Navigation => "navigation",
            KnownRole::Main => "main",
            KnownRole::Complementary => "complementary",
            KnownRole::ContentInfo => "contentinfo",
            KnownRole::Region => "region",
            KnownRole::Form => "form",
            KnownRole::Search => "search",
            KnownRole::Article => "article",
            KnownRole::Heading => "heading",
            KnownRole::Paragraph => "paragraph",
            KnownRole::List => "list",
            KnownRole::ListItem => "listitem",
            KnownRole::Separator => "separator",
            KnownRole::Blockquote => "blockquote",
            KnownRole::Figure => "figure",
            KnownRole::Image => "image",
            KnownRole::Button => "button",
            KnownRole::Link => "link",
            KnownRole::TextBox => "textbox",
            KnownRole::SearchBox => "searchbox",
            KnownRole::CheckBox => "checkbox",
            KnownRole::Radio => "radio",
            KnownRole::RadioGroup => "radiogroup",
            KnownRole::Switch => "switch",
            KnownRole::ComboBox => "combobox",
            KnownRole::ListBox => "listbox",
            KnownRole::Option => "option",
            KnownRole::Slider => "slider",
            KnownRole::SpinButton => "spinbutton",
            KnownRole::ProgressBar => "progressbar",
            KnownRole::Meter => "meter",
            KnownRole::Dialog => "dialog",
            KnownRole::Status => "status",
            KnownRole::Grid => "grid",
            KnownRole::Row => "row",
            KnownRole::Cell => "cell",
            KnownRole::RowGroup => "rowgroup",
            KnownRole::ColumnHeader => "columnheader",
            KnownRole::RowHeader => "rowheader",
            KnownRole::Table => "table",
            KnownRole::Tab => "tab",
            KnownRole::TabList => "tablist",
            KnownRole::TabPanel => "tabpanel",
            KnownRole::Menu => "menu",
            KnownRole::MenuItem => "menuitem",
            KnownRole::Summary => "summary",
            // HTML-AAM maps `<details>` to a group, which is what it is: a
            // thing that holds other things and can be opened.
            KnownRole::Group | KnownRole::Details => "group",
        }
    }

    /// The role an ARIA name spells, if this engine knows it.
    fn from_name(name: &str) -> Option<Self> {
        const ALL: &[KnownRole] = &[
            KnownRole::Document,
            KnownRole::Banner,
            KnownRole::Navigation,
            KnownRole::Main,
            KnownRole::Complementary,
            KnownRole::ContentInfo,
            KnownRole::Region,
            KnownRole::Form,
            KnownRole::Search,
            KnownRole::Article,
            KnownRole::Heading,
            KnownRole::Paragraph,
            KnownRole::List,
            KnownRole::ListItem,
            KnownRole::Group,
            KnownRole::Separator,
            KnownRole::Blockquote,
            KnownRole::Figure,
            KnownRole::Image,
            KnownRole::Button,
            KnownRole::Link,
            KnownRole::TextBox,
            KnownRole::SearchBox,
            KnownRole::CheckBox,
            KnownRole::Radio,
            KnownRole::RadioGroup,
            KnownRole::Switch,
            KnownRole::ComboBox,
            KnownRole::ListBox,
            KnownRole::Option,
            KnownRole::Slider,
            KnownRole::SpinButton,
            KnownRole::ProgressBar,
            KnownRole::Meter,
            KnownRole::Dialog,
            KnownRole::Status,
            KnownRole::Grid,
            KnownRole::Row,
            KnownRole::Cell,
            KnownRole::RowGroup,
            KnownRole::ColumnHeader,
            KnownRole::RowHeader,
            KnownRole::Table,
            KnownRole::Tab,
            KnownRole::TabList,
            KnownRole::TabPanel,
            KnownRole::Menu,
            KnownRole::MenuItem,
            KnownRole::Summary,
        ];
        ALL.iter()
            .copied()
            .find(|candidate| candidate.as_str().eq_ignore_ascii_case(name))
    }
}

impl Role {
    /// The role of an element: what the author declared, and failing that what
    /// HTML says the element is.
    pub fn of(document: &Document, id: NodeId, element: &Element) -> Self {
        if let Some(declared) = element.attr("role") {
            // ARIA takes a list and uses the first name it understands.
            for name in declared.split_ascii_whitespace() {
                if name.eq_ignore_ascii_case("presentation") || name.eq_ignore_ascii_case("none") {
                    return Role::Presentational;
                }
                if let Some(known) = KnownRole::from_name(name) {
                    return Role::Known(known);
                }
            }
            if let Some(first) = declared.split_ascii_whitespace().next() {
                return Role::Declared(first.into());
            }
        }
        implicit_role(document, id, element)
    }

    /// The role's name, as ARIA spells it.
    pub fn as_str(&self) -> &str {
        match self {
            Role::Generic => "generic",
            Role::Presentational => "presentation",
            Role::Known(known) => known.as_str(),
            Role::Declared(name) => name,
        }
    }

    /// Whether this box is anything in particular.
    pub fn is_generic(&self) -> bool {
        matches!(self, Role::Generic)
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The role HTML gives an element when the author declared none.
fn implicit_role(document: &Document, id: NodeId, element: &Element) -> Role {
    let local = &*element.name.local;
    if !element.name.ns.as_str().is_empty() && element.name.ns != alo_dom::Namespace::Html {
        // Foreign content: an `<svg>` is a picture, and nothing inside it has
        // a role of its own until somebody declares one.
        return if local == "svg" {
            Role::Known(KnownRole::Image)
        } else {
            Role::Generic
        };
    }

    let known = match local {
        "html" => KnownRole::Document,
        "nav" => KnownRole::Navigation,
        "main" => KnownRole::Main,
        "aside" => KnownRole::Complementary,
        "article" => KnownRole::Article,
        "search" => KnownRole::Search,
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => KnownRole::Heading,
        "p" => KnownRole::Paragraph,
        "ul" | "ol" | "menu" => KnownRole::List,
        "li" => KnownRole::ListItem,
        "hr" => KnownRole::Separator,
        "blockquote" => KnownRole::Blockquote,
        "figure" => KnownRole::Figure,
        "img" => KnownRole::Image,
        "button" => KnownRole::Button,
        "textarea" => KnownRole::TextBox,
        "select" => select_role(element),
        "option" => KnownRole::Option,
        "progress" => KnownRole::ProgressBar,
        "meter" => KnownRole::Meter,
        "dialog" => KnownRole::Dialog,
        "output" => KnownRole::Status,
        "summary" => KnownRole::Summary,
        "details" => KnownRole::Details,
        "fieldset" => KnownRole::Group,
        "input" => return input_role(element),
        "a" | "area" => {
            return if element.attr("href").is_some() {
                Role::Known(KnownRole::Link)
            } else {
                Role::Generic
            };
        }
        "form" => {
            // A form is a landmark only when it is named. An unnamed one is a
            // grouping, which is what ARIA says and what a screen reader wants.
            return if is_named(element) {
                Role::Known(KnownRole::Form)
            } else {
                Role::Generic
            };
        }
        "section" => {
            return if is_named(element) {
                Role::Known(KnownRole::Region)
            } else {
                Role::Generic
            };
        }
        "header" | "footer" => {
            // A header inside an article or a section is that thing's header,
            // not the page's, so it is not a landmark.
            return if is_scoped_to_the_page(document, id) {
                Role::Known(if local == "header" {
                    KnownRole::Banner
                } else {
                    KnownRole::ContentInfo
                })
            } else {
                Role::Generic
            };
        }
        _ => return Role::Generic,
    };
    Role::Known(known)
}

/// A `<select>` is a list to choose from, or a combo box when it is one line
/// high and not multiple — which is what a person sees and what HTML says.
fn select_role(element: &Element) -> KnownRole {
    let multiple = element.attr("multiple").is_some();
    let several_rows = element
        .attr("size")
        .and_then(|size| size.trim().parse::<u32>().ok())
        .is_some_and(|size| size > 1);
    if multiple || several_rows {
        KnownRole::ListBox
    } else {
        KnownRole::ComboBox
    }
}

/// An `<input>`'s role is its `type`, which is the author declaring it.
fn input_role(element: &Element) -> Role {
    let kind = element
        .attr("type")
        .map_or_else(|| "text".to_owned(), str::to_ascii_lowercase);
    let known = match &*kind {
        "button" | "submit" | "reset" | "image" => KnownRole::Button,
        "checkbox" => KnownRole::CheckBox,
        "radio" => KnownRole::Radio,
        "range" => KnownRole::Slider,
        "number" => KnownRole::SpinButton,
        "search" => KnownRole::SearchBox,
        "text" | "tel" | "url" | "email" => KnownRole::TextBox,
        // `hidden`, and the date and colour pickers, which HTML gives no role:
        // there is nothing true to say, and inventing one is what this file
        // exists to prevent.
        _ => return Role::Generic,
    };
    Role::Known(known)
}

/// Whether the author gave this element a name outright.
fn is_named(element: &Element) -> bool {
    ["aria-label", "aria-labelledby", "title"]
        .iter()
        .any(|name| {
            element
                .attr(name)
                .is_some_and(|value| !value.trim().is_empty())
        })
}

/// Whether an element is the page's own rather than some section's.
fn is_scoped_to_the_page(document: &Document, id: NodeId) -> bool {
    const SECTIONING: &[&str] = &["article", "aside", "main", "nav", "section"];
    let mut current = document.parent(id);
    while let Some(node) = current {
        if let Some(element) = document.element(node)
            && SECTIONING.iter().any(|name| element.name.is_html(name))
        {
            return false;
        }
        current = document.parent(node);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::parse_document;

    /// The role of the element with this `id` attribute.
    fn role_of(html: &str, wanted: &str) -> String {
        let document = parse_document(html);
        let id = document
            .descendants(document.root())
            .find(|id| {
                document
                    .element(*id)
                    .is_some_and(|element| element.attr("id") == Some(wanted))
            })
            .unwrap_or_else(|| panic!("no element with id={wanted}"));
        let element = document.element(id).expect("an element");
        Role::of(&document, id, element).to_string()
    }

    #[test]
    fn the_role_attribute_is_the_author_saying_it_outright() {
        assert_eq!(role_of("<div id=x role=button>b</div>", "x"), "button");
        assert_eq!(role_of("<div id=x role=row>r</div>", "x"), "row");
        assert_eq!(
            role_of("<ul id=x role=tablist></ul>", "x"),
            "tablist",
            "and it wins over what the element would have been",
        );
    }

    #[test]
    fn a_role_this_engine_does_not_know_is_kept_rather_than_dropped() {
        assert_eq!(
            role_of("<div id=x role=doc-endnote>e</div>", "x"),
            "doc-endnote"
        );
        assert_eq!(Role::Declared("doc-endnote".into()).as_str(), "doc-endnote",);
    }

    #[test]
    fn a_list_of_roles_takes_the_first_one_that_is_understood() {
        assert_eq!(
            role_of("<div id=x role='nonsense button'>b</div>", "x"),
            "button",
        );
    }

    #[test]
    fn presentational_is_the_author_saying_this_is_nothing() {
        assert_eq!(
            role_of("<ul id=x role=presentation></ul>", "x"),
            "presentation"
        );
        assert_eq!(role_of("<ul id=x role=none></ul>", "x"), "presentation");
    }

    #[test]
    fn html_gives_an_element_the_role_the_standard_says_it_has() {
        let cases = [
            ("<nav id=x></nav>", "navigation"),
            ("<main id=x></main>", "main"),
            ("<aside id=x></aside>", "complementary"),
            ("<article id=x></article>", "article"),
            ("<h2 id=x>t</h2>", "heading"),
            ("<p id=x>t</p>", "paragraph"),
            ("<ul id=x></ul>", "list"),
            ("<ol id=x></ol>", "list"),
            ("<li id=x>i</li>", "listitem"),
            ("<hr id=x>", "separator"),
            ("<blockquote id=x>q</blockquote>", "blockquote"),
            ("<figure id=x></figure>", "figure"),
            ("<img id=x alt=a>", "image"),
            ("<button id=x>b</button>", "button"),
            ("<textarea id=x></textarea>", "textbox"),
            ("<progress id=x></progress>", "progressbar"),
            ("<dialog id=x></dialog>", "dialog"),
            ("<div id=x></div>", "generic"),
            ("<span id=x></span>", "generic"),
        ];
        for (html, expected) in cases {
            assert_eq!(role_of(html, "x"), expected, "{html}");
        }
    }

    #[test]
    fn an_anchor_is_a_link_only_when_it_goes_somewhere() {
        assert_eq!(role_of("<a id=x href='/'>go</a>", "x"), "link");
        assert_eq!(role_of("<a id=x>nowhere</a>", "x"), "generic");
    }

    #[test]
    fn an_inputs_role_is_its_type() {
        let cases = [
            ("<input id=x>", "textbox"),
            ("<input id=x type=text>", "textbox"),
            ("<input id=x type=search>", "searchbox"),
            ("<input id=x type=checkbox>", "checkbox"),
            ("<input id=x type=radio>", "radio"),
            ("<input id=x type=range>", "slider"),
            ("<input id=x type=number>", "spinbutton"),
            ("<input id=x type=submit>", "button"),
            ("<input id=x type=hidden>", "generic"),
            ("<input id=x type=color>", "generic"),
        ];
        for (html, expected) in cases {
            assert_eq!(role_of(html, "x"), expected, "{html}");
        }
    }

    #[test]
    fn a_select_is_a_combo_box_until_it_shows_several_rows() {
        assert_eq!(role_of("<select id=x></select>", "x"), "combobox");
        assert_eq!(role_of("<select id=x size=4></select>", "x"), "listbox");
        assert_eq!(role_of("<select id=x multiple></select>", "x"), "listbox");
    }

    #[test]
    fn a_section_or_a_form_is_a_landmark_only_when_it_is_named() {
        assert_eq!(role_of("<section id=x></section>", "x"), "generic");
        assert_eq!(
            role_of("<section id=x aria-label='Invoices'></section>", "x"),
            "region",
        );
        assert_eq!(role_of("<form id=x></form>", "x"), "generic");
        assert_eq!(role_of("<form id=x title='Sign in'></form>", "x"), "form");
        assert_eq!(
            role_of("<section id=x aria-label='  '></section>", "x"),
            "generic",
            "a name of nothing is not a name",
        );
    }

    #[test]
    fn a_header_is_the_pages_only_when_it_is_not_inside_a_section() {
        assert_eq!(
            role_of("<body><header id=x></header></body>", "x"),
            "banner"
        );
        assert_eq!(
            role_of("<article><header id=x></header></article>", "x"),
            "generic",
        );
        assert_eq!(
            role_of("<body><footer id=x></footer></body>", "x"),
            "contentinfo"
        );
        assert_eq!(
            role_of("<section><div><footer id=x></footer></div></section>", "x"),
            "generic",
            "however deep inside the section it is",
        );
    }

    #[test]
    fn foreign_content_is_a_picture_and_its_insides_are_nothing() {
        assert_eq!(role_of("<svg id=x></svg>", "x"), "image");
        assert_eq!(role_of("<svg><circle id=x></circle></svg>", "x"), "generic");
    }

    #[test]
    fn a_role_is_matched_however_it_is_capitalised() {
        assert_eq!(role_of("<div id=x role=BUTTON>b</div>", "x"), "button");
        assert_eq!(role_of("<div id=x role=None>b</div>", "x"), "presentation");
    }

    #[test]
    fn generic_says_that_it_is_generic() {
        assert!(Role::Generic.is_generic());
        assert!(!Role::Known(KnownRole::Button).is_generic());
    }
}
