/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a form's submission sends, and where (ADR 0051 §§ 4 and 5).
//!
//! HTML's *form submission algorithm*, as far as it is a rule about the
//! document. Whoever runs the click runs the steps around these: checking
//! the controls ([`crate::constraint`]), firing `invalid` and `submit` on a
//! page that runs script, and asking for the navigation. This is the part
//! both callers share:
//!
//! - [`plan`] — the **entry list**, then the **method**, **action**,
//!   **enctype** and **target**, from the form or from the submitter's
//!   `formmethod`, `formaction`, `formenctype` and `formtarget` where it sets
//!   them, in HTML's order;
//! - [`novalidate`] — whether the controls are checked at all.
//!
//! The action is kept **as written**. Resolving it against the document's
//! base URL is the navigation's, as a link's `href` is
//! (`alo-bindings`' `navigating`), and encoding the entries is the browser
//! process's (§ 5), so that the bytes of a file a person chose never reach
//! a renderer.
//!
//! # The entry list, as Chrome 154 builds it
//!
//! Measured with `new FormData(form, submitter)` on a probe page, and on
//! `web-a-form` by intercepting what it sent. Each control the form owns, in
//! tree order, gives its name and value, and gives nothing when:
//!
//! - it is disabled, or in a disabled `fieldset` and not its first legend;
//! - it is a button that is not the submitter;
//! - it is a checkbox or radio that is not checked;
//! - it has no `name`, or an empty one.
//!
//! A `<select>` gives each selected option that is not disabled. A file
//! field gives one entry naming no file, since no file is ever chosen
//! without a window to choose it in (§ 8). A hidden input called
//! `_charset_` gives `UTF-8`. A submit input with no `value` gives
//! `Submit`, the label Chrome draws on it. A control inside a `datalist`
//! **is** sent, though HTML says otherwise; Chrome sends it, and this
//! follows what a server would receive.
//!
//! # Refused by name
//!
//! Each of these stops the submission and says which, rather than sending
//! something a person did not mean ([`Unsubmittable`]):
//!
//! - an `accept-charset` whose first label is not UTF-8. Every document
//!   here is UTF-8, and other encodings are the legacy tail;
//! - a control whose value is not read yet ([`crate::value::is_unheld`]);
//! - an `input type=image` as the submitter. It sends the coordinates of a
//!   click, and an agent has none (ADR 0002);
//! - `method="dialog"`, which closes a `<dialog>`, which is not built;
//! - a target naming another window, as for a link.

use crate::document::Document;
use crate::kind::{self, Kind};
use crate::node::{Element, NodeId};
use crate::owner;
use crate::value;
use core::fmt;

/// How a form is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// `get`: the entries are the URL's query.
    Get,
    /// `post`: the entries are the body.
    Post,
}

impl Method {
    /// Its name in a request.
    pub const fn name(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
        }
    }
}

/// How a form's entries are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Enctype {
    /// `application/x-www-form-urlencoded`.
    UrlEncoded,
    /// `multipart/form-data`.
    Multipart,
    /// `text/plain`.
    TextPlain,
}

impl Enctype {
    /// Its media type.
    pub const fn media_type(self) -> &'static str {
        match self {
            Enctype::UrlEncoded => "application/x-www-form-urlencoded",
            Enctype::Multipart => "multipart/form-data",
            Enctype::TextPlain => "text/plain",
        }
    }
}

/// What one entry holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// Text.
    Text(String),
    /// A file field's entry when no file is chosen: a file with no name, no
    /// type and nothing in it.
    NoFile,
}

/// One name and value in the entry list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The control's name.
    pub name: String,
    /// What it sends.
    pub value: Value,
}

impl Entry {
    fn text(name: &str, value: impl Into<String>) -> Self {
        Entry {
            name: name.to_owned(),
            value: Value::Text(value.into()),
        }
    }
}

/// What a submission sends, and where, before anyone has resolved or
/// encoded it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// How.
    pub method: Method,
    /// Where to, as written. The empty string is the document's own URL.
    pub action: String,
    /// How the entries are written. A `GET`'s is ignored, as HTML says.
    pub enctype: Enctype,
    /// The entries, in tree order.
    pub entries: Vec<Entry>,
    /// Whether the form's `rel` says `noreferrer`.
    pub no_referrer: bool,
}

/// Why a form was not submitted, where this engine refuses rather than
/// guesses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unsubmittable {
    /// Its `accept-charset` asks for an encoding other than UTF-8.
    Charset {
        /// What it says.
        charset: String,
    },
    /// A control's value is one this engine does not read yet.
    Unheld {
        /// The control.
        control: NodeId,
        /// Its type, as HTML names it.
        kind: String,
    },
    /// The submitter is an image button, which sends a click's coordinates.
    ImageButton,
    /// Its method is `dialog`.
    Dialog,
    /// Its target names another window.
    AnotherWindow {
        /// The target.
        target: String,
    },
}

impl fmt::Display for Unsubmittable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unsubmittable::Charset { charset } => write!(
                f,
                "the form asks to be sent as {charset:?}, and this engine sends only UTF-8"
            ),
            Unsubmittable::Unheld { kind, .. } => write!(
                f,
                "the form holds a {kind} field, and this engine does not read what one holds yet"
            ),
            Unsubmittable::ImageButton => f.write_str(
                "an image button sends where it was clicked, and an agent clicks nowhere; a \
                 person's point is not sent yet (queue item 556)",
            ),
            Unsubmittable::Dialog => {
                f.write_str("the form closes a dialog, and dialogs are not built")
            }
            Unsubmittable::AnotherWindow { target } => write!(
                f,
                "the form is sent to another window ({target:?}), and opening one is queue item \
                 118"
            ),
        }
    }
}

impl std::error::Error for Unsubmittable {}

/// The label Chrome 154 draws on, and sends for, a submit input with no
/// `value`.
const SUBMIT_LABEL: &str = "Submit";

/// The labels that name UTF-8, from the Encoding standard.
const UTF_8: &[&str] = &[
    "unicode-1-1-utf-8",
    "unicode11utf8",
    "unicode20utf8",
    "utf-8",
    "utf8",
    "x-unicode20utf8",
];

/// Whether submitting `form` from `submitter` skips checking its controls:
/// the form's `novalidate`, or the submitter's `formnovalidate`.
pub fn novalidate(document: &Document, form: NodeId, submitter: Option<NodeId>) -> bool {
    let says = |node: NodeId, name: &str| {
        document
            .element(node)
            .is_some_and(|element| element.attr(name).is_some())
    };
    says(form, "novalidate") || submitter.is_some_and(|button| says(button, "formnovalidate"))
}

/// What submitting `form` from `submitter` sends, and where, in HTML's
/// order: the encoding, the entry list, the method, the action, the enctype
/// and the target.
///
/// # Errors
///
/// [`Unsubmittable`], for the reasons the module lists.
pub fn plan(
    document: &Document,
    form: NodeId,
    submitter: Option<NodeId>,
) -> Result<Plan, Unsubmittable> {
    let element = document.element(form);
    let attr = |name: &str| element.and_then(|form| form.attr(name));
    if let Some(charset) = attr("accept-charset")
        && let Some(first) = charset.split_ascii_whitespace().next()
        && !UTF_8.iter().any(|label| first.eq_ignore_ascii_case(label))
    {
        return Err(Unsubmittable::Charset {
            charset: charset.to_owned(),
        });
    }
    let entries = entries(document, form, submitter)?;
    // The submitter's own attribute, where it is a submit button and says
    // one, or else the form's.
    let chosen = |own: &str, forms: &str| {
        submitter
            .and_then(|button| document.element(button))
            .filter(|button| kind::is_submit_button(button))
            .and_then(|button| button.attr(own))
            .or_else(|| attr(forms))
    };
    let method = chosen("formmethod", "method").unwrap_or_default();
    if method.eq_ignore_ascii_case("dialog") {
        return Err(Unsubmittable::Dialog);
    }
    let method = if method.eq_ignore_ascii_case("post") {
        Method::Post
    } else {
        Method::Get
    };
    let action = chosen("formaction", "action")
        .unwrap_or_default()
        .to_owned();
    let enctype = chosen("formenctype", "enctype").unwrap_or_default();
    let enctype = [Enctype::Multipart, Enctype::TextPlain]
        .into_iter()
        .find(|known| enctype.eq_ignore_ascii_case(known.media_type()))
        .unwrap_or(Enctype::UrlEncoded);
    let target = chosen("formtarget", "target").or_else(|| base_target(document));
    if let Some(target) = target
        && !is_this_tab(target)
    {
        return Err(Unsubmittable::AnotherWindow {
            target: target.to_owned(),
        });
    }
    let no_referrer = attr("rel").is_some_and(|rel| {
        rel.split_ascii_whitespace()
            .any(|token| token.eq_ignore_ascii_case("noreferrer"))
    });
    Ok(Plan {
        method,
        action,
        enctype,
        entries,
        no_referrer,
    })
}

/// HTML's *constructing the entry list* for `form`, submitted from
/// `submitter`.
///
/// # Errors
///
/// [`Unsubmittable::Unheld`] and [`Unsubmittable::ImageButton`].
pub fn entries(
    document: &Document,
    form: NodeId,
    submitter: Option<NodeId>,
) -> Result<Vec<Entry>, Unsubmittable> {
    let mut entries = Vec::new();
    for control in owner::controls(document, form) {
        let Some(element) = document.element(control) else {
            continue;
        };
        if crate::disabled::is_disabled(document, control, element) {
            continue;
        }
        let kind = kind::of(element).or_else(|| kind::of_button(element));
        let is_button = element.name.is_html("button") || kind.is_some_and(Kind::is_button);
        if is_button && Some(control) != submitter {
            continue;
        }
        if kind == Some(Kind::Image) {
            return Err(Unsubmittable::ImageButton);
        }
        if matches!(kind, Some(Kind::Checkbox | Kind::Radio)) && !value::checked(document, control)
        {
            continue;
        }
        let Some(name) = element.attr("name").filter(|name| !name.is_empty()) else {
            continue;
        };
        if let Some(unheld) = value::is_unheld(document, control) {
            return Err(Unsubmittable::Unheld {
                control,
                kind: unheld.to_owned(),
            });
        }
        entries.extend(entries_of(document, control, element, kind, name));
    }
    Ok(entries)
}

/// What one control that sends something sends.
fn entries_of(
    document: &Document,
    control: NodeId,
    element: &Element,
    kind: Option<Kind>,
    name: &str,
) -> Vec<Entry> {
    if element.name.is_html("select") {
        return value::selected(document, control)
            .into_iter()
            .filter(|option| !crate::disabled::is_disabled_node(document, *option))
            .map(|option| Entry::text(name, value::option_value(document, option)))
            .collect();
    }
    let entry = match kind {
        Some(Kind::File) => Entry {
            name: name.to_owned(),
            value: Value::NoFile,
        },
        Some(Kind::Hidden) if name.eq_ignore_ascii_case("_charset_") => Entry::text(name, "UTF-8"),
        Some(Kind::Submit) if element.name.is_html("input") && element.attr("value").is_none() => {
            Entry::text(name, SUBMIT_LABEL)
        }
        _ if element.name.is_html("button") => {
            Entry::text(name, element.attr("value").unwrap_or_default())
        }
        _ => Entry::text(name, value::value(document, control)),
    };
    vec![entry]
}

/// The `target` of the first `<base>` that has one.
fn base_target(document: &Document) -> Option<&str> {
    document.descendants(document.root()).find_map(|node| {
        document
            .element(node)
            .filter(|element| element.name.is_html("base"))
            .and_then(|element| element.attr("target"))
    })
}

/// Whether a target names this tab: nothing, `_self`, `_parent` or `_top`,
/// which with no frames (item 86) are all the page's own tab.
fn is_this_tab(target: &str) -> bool {
    target.is_empty()
        || ["_self", "_parent", "_top"]
            .iter()
            .any(|own| target.eq_ignore_ascii_case(own))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_document;

    fn named(document: &Document, name: &str) -> Option<NodeId> {
        document.descendants(document.root()).find(|node| {
            document
                .element(*node)
                .is_some_and(|element| element.attr("name") == Some(name))
        })
    }

    fn with_id(document: &Document, id: &str) -> Option<NodeId> {
        document.descendants(document.root()).find(|node| {
            document
                .element(*node)
                .is_some_and(|element| element.attr("id") == Some(id))
        })
    }

    fn written(entries: &[Entry]) -> Vec<String> {
        entries
            .iter()
            .map(|entry| match &entry.value {
                Value::Text(text) => format!("{}={text}", entry.name),
                Value::NoFile => format!("{}=<no file>", entry.name),
            })
            .collect()
    }

    /// The probe form Chrome 154 was given, and what `FormData` gave back
    /// for it with each submitter.
    const PROBE: &str = "<form id=f2>\
        <input type=submit name=go><input type=submit name=go2 value='Send it'>\
        <input type=reset name=rs><button name=btn value=v type=submit>B</button>\
        <button name=bt2>B2</button>\
        <input name=plain value=' x '><input type=email name=em value=' a@b.c '>\
        <input type=url name=ur value=' https://a.b '>\
        <input type=number name=nu value=1e3><input type=number name=nu2 value=abc>\
        <input type=date name=da value=2026-13-01>\
        <input type=checkbox name=cb checked><input type=checkbox name=cb2 checked value=''>\
        <input type=radio name=rr checked>\
        <select name=sel><option>  one   two </option><option>b</option></select>\
        <select name=sel2><option disabled>x</option><option>y</option></select>\
        <select name=sel3 multiple><option selected>m1</option>\
        <option selected value=m2v>m2</option><option>m3</option></select>\
        <select name=sel4><option value=a selected>a</option><option value=b selected>b</option>\
        </select><select name=sel5></select><select name=sel6 size=3><option>q</option></select>\
        <input type=file name=file><input type=hidden name=_charset_>\
        <input type=hidden name=hid value=hv><textarea name=tex>\nline1\nline2</textarea>\
        <fieldset disabled><input name=infs value=1><legend><input name=inleg value=2></legend>\
        </fieldset><input name='' value=noname><input value=nonameattr>\
        <output name=out>o</output><input type=checkbox name=cbx>\
        <datalist><input name=indl value=dl></datalist><input name=t3 required></form>\
        <input form=f2 name=outside value=o1><form id=f3><input name=x form=f2 value=x1></form>";

    const PROBE_ENTRIES: &[&str] = &[
        "plain= x ",
        "em=a@b.c",
        "ur=https://a.b",
        "nu=1e3",
        "nu2=",
        "da=",
        "cb=on",
        "cb2=",
        "rr=on",
        "sel=one two",
        "sel2=y",
        "sel3=m1",
        "sel3=m2v",
        "sel4=b",
        "file=<no file>",
        "_charset_=UTF-8",
        "hid=hv",
        "tex=line1\nline2",
        "inleg=2",
        "indl=dl",
        "t3=",
        "outside=o1",
        "x=x1",
    ];

    #[test]
    fn the_entry_list_is_the_one_chrome_builds() {
        let document = parse_document(PROBE);
        let Some(form) = with_id(&document, "f2") else {
            panic!("the form");
        };
        for (submitter, first) in [
            (None, None),
            (Some("go"), Some("go=Submit")),
            (Some("go2"), Some("go2=Send it")),
            (Some("btn"), Some("btn=v")),
            (Some("bt2"), Some("bt2=")),
        ] {
            let submitter = submitter.and_then(|name| named(&document, name));
            let Ok(entries) = entries(&document, form, submitter) else {
                panic!("the probe submits");
            };
            let expected: Vec<String> = first
                .into_iter()
                .chain(PROBE_ENTRIES.iter().copied())
                .map(str::to_owned)
                .collect();
            assert_eq!(written(&entries), expected, "{submitter:?}");
        }
    }

    #[test]
    fn a_forms_method_action_and_enctype_are_its_own_or_its_submitters() {
        let document = parse_document(
            "<form id=f method=POST action=/post enctype=TEXT/PLAIN>\
             <button name=b>b</button>\
             <button name=over formmethod=get formaction=/get formenctype=multipart/form-data>\
             o</button><input type=reset name=r formmethod=get></form>\
             <form id=g method=nonsense enctype=nonsense></form>",
        );
        let (Some(f), Some(g)) = (with_id(&document, "f"), with_id(&document, "g")) else {
            panic!("the forms");
        };
        let Ok(own) = plan(&document, f, named(&document, "b")) else {
            panic!("it submits");
        };
        assert_eq!(
            (own.method, own.action.as_str(), own.enctype),
            (Method::Post, "/post", Enctype::TextPlain)
        );
        let Ok(over) = plan(&document, f, named(&document, "over")) else {
            panic!("it submits");
        };
        assert_eq!(
            (over.method, over.action.as_str(), over.enctype),
            (Method::Get, "/get", Enctype::Multipart)
        );
        let Ok(none) = plan(&document, g, None) else {
            panic!("it submits");
        };
        assert_eq!(
            (none.method, none.action.as_str(), none.enctype),
            (Method::Get, "", Enctype::UrlEncoded)
        );
        assert!(!own.no_referrer);
    }

    #[test]
    fn what_this_engine_does_not_build_is_refused_by_name() {
        let refused = |markup: &str, submitter: Option<&str>| {
            let document = parse_document(markup);
            let form = with_id(&document, "f")?;
            let submitter = submitter.and_then(|name| named(&document, name));
            plan(&document, form, submitter).err()
        };
        assert_eq!(
            refused("<form id=f accept-charset='iso-8859-1 utf-8'></form>", None),
            Some(Unsubmittable::Charset {
                charset: "iso-8859-1 utf-8".to_owned()
            })
        );
        assert_eq!(
            refused("<form id=f accept-charset='UTF-8 iso-8859-1'></form>", None),
            None
        );
        assert_eq!(
            refused("<form id=f method=dialog></form>", None),
            Some(Unsubmittable::Dialog)
        );
        assert_eq!(
            refused("<form id=f target=_blank></form>", None),
            Some(Unsubmittable::AnotherWindow {
                target: "_blank".to_owned()
            })
        );
        assert_eq!(
            refused(
                "<base target=results><form id=f><button name=b formtarget=_self>b</button>\
                 </form>",
                Some("b")
            ),
            None,
            "the submitter's own target wins over the base's"
        );
        assert_eq!(
            refused("<base target=results><form id=f></form>", None),
            Some(Unsubmittable::AnotherWindow {
                target: "results".to_owned()
            })
        );
        assert_eq!(
            refused("<form id=f><input type=image name=i></form>", Some("i")),
            Some(Unsubmittable::ImageButton)
        );
        assert_eq!(
            refused("<form id=f><input type=image name=i></form>", None),
            None,
            "an image button that did not submit sends nothing"
        );
        assert!(matches!(
            refused("<form id=f><input type=time name=t value=10:00></form>", None),
            Some(Unsubmittable::Unheld { kind, .. }) if kind == "time"
        ));
        assert!(matches!(
            refused("<form id=f><input type=range name=r></form>", None),
            Some(Unsubmittable::Unheld { kind, .. }) if kind == "range"
        ));
        assert_eq!(
            refused("<form id=f><input type=time name=t></form>", None),
            None,
            "an empty time field submits, as web-a-form's does"
        );
    }

    #[test]
    fn novalidate_is_the_forms_or_the_submitters() {
        let document = parse_document(
            "<form id=f><button name=a>a</button><button name=b formnovalidate>b</button></form>\
             <form id=g novalidate></form>",
        );
        let (Some(f), Some(g)) = (with_id(&document, "f"), with_id(&document, "g")) else {
            panic!("the forms");
        };
        assert!(!novalidate(&document, f, named(&document, "a")));
        assert!(novalidate(&document, f, named(&document, "b")));
        assert!(novalidate(&document, g, None));
    }
}
