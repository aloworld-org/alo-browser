/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The lists' hiding sheet put into each document a card's page commits
//! (queue item 806, ADR 0094's amendment of 6 November 2026) — through
//! Chromium's own style sheet commands, never a script of ours in the page.
//!
//! As a frame of the tab commits a document from the network, the
//! browser process asks [`crate::blocking::Blocker::hiding`] for that
//! document's address and, when the lists name anything, has Chromium make
//! the document a style sheet of its own and fill it: `DOM.enable` and
//! `CSS.enable` (which a sheet needs made, measured against Chrome 157),
//! `CSS.createStyleSheet` for the frame, and — once its id is answered —
//! `CSS.setStyleSheetText` and `CSS.disable`, so Chromium does not go on
//! telling of every sheet the page adds. Measured: the sheet stays after
//! `CSS.disable`; made at the commit, it is in place before the first paint
//! on every page tried.
//!
//! Chromium puts such a sheet in the document as a `<style>` element of
//! its head, which a page's script can see and remove: what is hidden is
//! the page's to undo, and nothing of ours runs there.
//!
//! This keeps which commands are in flight and sends each next one through
//! what the tab hands it; the tab hands each answer here
//! ([`Cosmetic::answered`]).

use crate::chrome::Failed;
use crate::chrome_protocol::{self, Command, Declined};
use std::collections::{HashMap, HashSet};

/// How a command is sent through a session: its id, or why it was not.
pub type Send<'a> = &'a mut dyn FnMut(&str, &Command) -> Result<u64, Failed>;

/// A sheet on its way: the session it goes through, its frame and rules.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Sheet {
    session: String,
    frame: String,
    rules: String,
}

/// The sheets being made, and the commands whose answers nothing needs.
#[derive(Debug, Default)]
pub struct Cosmetic {
    /// Each `CSS.enable` in flight, and the sheet to make once it answers:
    /// Chromium refuses a sheet asked for before then (*No target
    /// stylesheet found*, measured against Chrome 157, which answers
    /// `CSS.enable` only once the document's own sheets are read).
    enabling: HashMap<u64, Sheet>,
    /// Each `CSS.createStyleSheet` in flight, and the sheet's.
    making: HashMap<u64, Sheet>,
    /// Commands sent whose answers are taken and dropped.
    quiet: HashSet<u64>,
}

impl Cosmetic {
    /// Begin a sheet holding `rules` for the document `frame` holds,
    /// through `session`: `DOM.enable`, then `CSS.enable`, whose answer
    /// makes it.
    ///
    /// # Errors
    ///
    /// What `send` said.
    pub fn begin(
        &mut self,
        session: &str,
        frame: &str,
        rules: String,
        send: Send<'_>,
    ) -> Result<(), Failed> {
        let dom = send(session, &Command::ReadTheDom)?;
        self.quiet.insert(dom);
        let css = send(session, &Command::ReadStyles)?;
        self.enabling.insert(
            css,
            Sheet {
                session: session.to_owned(),
                frame: frame.to_owned(),
                rules,
            },
        );
        Ok(())
    }

    /// Whether `id` is an answer this waits for.
    pub fn awaits(&self, id: u64) -> bool {
        self.enabling.contains_key(&id) || self.making.contains_key(&id) || self.quiet.contains(&id)
    }

    /// Chrome answered `id` with `result`: the CSS domain on, the sheet is
    /// asked for; the sheet made, it is filled and the domain turned off.
    /// A sheet that could not be made — its frame gone, its document
    /// replaced — is let be: the next commit makes the next.
    ///
    /// # Errors
    ///
    /// What `send` said.
    pub fn answered(
        &mut self,
        id: u64,
        result: &Result<Vec<u8>, Declined>,
        send: Send<'_>,
    ) -> Result<(), Failed> {
        if self.quiet.remove(&id) {
            return Ok(());
        }
        if let Some(sheet) = self.enabling.remove(&id) {
            if result.is_err() {
                return Ok(());
            }
            let asked = Command::MakeSheet {
                frame: sheet.frame.clone(),
            };
            let id = send(&sheet.session, &asked)?;
            self.making.insert(id, sheet);
            return Ok(());
        }
        let Some(sheet) = self.making.remove(&id) else {
            return Ok(());
        };
        let made = result
            .as_ref()
            .ok()
            .and_then(|made| chrome_protocol::sheet_made(made).ok());
        if let Some(made) = made {
            let fill = Command::FillSheet {
                sheet: made,
                text: sheet.rules,
            };
            let filled = send(&sheet.session, &fill)?;
            self.quiet.insert(filled);
        }
        let off = send(&sheet.session, &Command::StopReadingStyles)?;
        self.quiet.insert(off);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What was sent, by session and method, and a sender numbering each.
    fn sender(
        sent: &mut Vec<(String, Command)>,
    ) -> impl FnMut(&str, &Command) -> Result<u64, Failed> + '_ {
        move |session, command| {
            sent.push((session.to_owned(), command.clone()));
            Ok(u64::try_from(sent.len()).unwrap_or_default())
        }
    }

    fn methods(sent: &[(String, Command)]) -> Vec<&'static str> {
        sent.iter().map(|(_, command)| command.method()).collect()
    }

    #[test]
    fn a_sheet_is_made_once_the_css_domain_answers_then_filled_and_turned_off() {
        let mut cosmetic = Cosmetic::default();
        let mut sent = Vec::new();
        let rules = ".ad{display:none!important}\n".to_owned();
        cosmetic
            .begin("S", "F", rules.clone(), &mut sender(&mut sent))
            .unwrap();
        assert_eq!(methods(&sent), vec!["DOM.enable", "CSS.enable"]);
        assert!(cosmetic.awaits(1) && cosmetic.awaits(2) && !cosmetic.awaits(3));
        cosmetic
            .answered(1, &Ok(b"{}".to_vec()), &mut sender(&mut sent))
            .unwrap();
        assert_eq!(sent.len(), 2, "DOM's answer asks nothing");
        cosmetic
            .answered(2, &Ok(b"{}".to_vec()), &mut sender(&mut sent))
            .unwrap();
        assert_eq!(
            sent.last(),
            Some(&(
                "S".to_owned(),
                Command::MakeSheet {
                    frame: "F".to_owned()
                }
            ))
        );
        cosmetic
            .answered(
                3,
                &Ok(br#"{"styleSheetId":"s-1"}"#.to_vec()),
                &mut sender(&mut sent),
            )
            .unwrap();
        assert_eq!(
            methods(&sent),
            vec![
                "DOM.enable",
                "CSS.enable",
                "CSS.createStyleSheet",
                "CSS.setStyleSheetText",
                "CSS.disable"
            ]
        );
        assert_eq!(
            sent.get(3).map(|(_, command)| command.clone()),
            Some(Command::FillSheet {
                sheet: "s-1".to_owned(),
                text: rules
            })
        );
        assert!(!cosmetic.awaits(3), "each answer once");
        assert!(
            cosmetic.awaits(4) && cosmetic.awaits(5),
            "the last two quietly"
        );
    }

    #[test]
    fn a_sheet_not_made_is_filled_with_nothing_and_the_domain_still_turned_off() {
        let mut cosmetic = Cosmetic::default();
        let mut sent = Vec::new();
        cosmetic
            .begin(
                "S",
                "F",
                "x{display:none!important}\n".to_owned(),
                &mut sender(&mut sent),
            )
            .unwrap();
        let declined = Declined {
            code: -32000,
            message: "No target stylesheet found".to_owned(),
        };
        // The domain refused: nothing more.
        cosmetic
            .answered(2, &Err(declined.clone()), &mut sender(&mut sent))
            .unwrap();
        assert_eq!(sent.len(), 2);
        cosmetic
            .begin(
                "S",
                "F",
                "x{display:none!important}\n".to_owned(),
                &mut sender(&mut sent),
            )
            .unwrap();
        cosmetic
            .answered(4, &Ok(b"{}".to_vec()), &mut sender(&mut sent))
            .unwrap();
        cosmetic
            .answered(5, &Err(declined), &mut sender(&mut sent))
            .unwrap();
        assert_eq!(methods(&sent)[4..], ["CSS.createStyleSheet", "CSS.disable"]);
    }
}
