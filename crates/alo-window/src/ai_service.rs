/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The AI service: the one interface alo asks a model through, whoever's
//! it is (ADR 0106 § 1, queue item 904).
//!
//! A search's answer ([`crate::conversation_answer`], ADR 0095) and alo's
//! own agent ([`crate::conversation_agent`], ADR 0096) hold an
//! [`AiService`] and nothing of whose model it is. Behind it, an
//! [`AiConnector`]: today one, `OpenAI`'s Responses API
//! ([`crate::openai_answer`]), whose wire formats are its own
//! ([`crate::answer_protocol`], [`crate::agent_protocol`]). Which is made is
//! configuration ([`crate::services_made`]).
//!
//! # Named to nobody
//!
//! What a person reads calls it [`AI_SERVICE`], never by name (ADR 0106
//! § 5). [`AiConnector::named`] is for the terminal alone, which is the
//! person running alo's own.

use crate::agent_protocol::{self, Asking};
use crate::answer_protocol::Spent;
use alo_net::Pool;
use alo_net::cause::Identities;
use alo_renderer::results::Found;
use alo_renderer::results_answer::Answer;
use core::fmt;
use std::sync::Arc;

/// What a sentence a person reads calls the AI service, after a colon.
pub const AI_SERVICE: &str = "the AI service";

/// What came of asking for an answer: the answer, or why not in its
/// [`Answer::Unanswered`]; and what it cost, if the service said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    /// The answer, or why there is none.
    pub answer: Answer,
    /// What it cost.
    pub spent: Option<Spent>,
}

/// One AI service, asked through the interface.
pub trait AiConnector: fmt::Debug + Send + Sync {
    /// Who answers and with which model, for the terminal alone:
    /// *`OpenAI`'s gpt-6.1-terra*.
    fn named(&self) -> String;

    /// The answer to `question` from `sources` (ADR 0095): what came of it,
    /// an answer not had saying why in a sentence a person reads.
    fn answer(
        &self,
        pool: &mut Pool,
        identities: &mut Identities,
        question: &str,
        sources: &[Found],
    ) -> Asked;

    /// The agent's next step for `asking` (ADR 0096).
    ///
    /// # Errors
    ///
    /// Why there is none, in the words a person reads after *alo could not
    /// go on:* — never the key.
    fn step(
        &self,
        pool: &mut Pool,
        identities: &mut Identities,
        asking: &Asking<'_>,
    ) -> Result<agent_protocol::Read, String>;
}

/// The AI service alo asks: any connector, held so every thread that asks
/// it may hold it too.
#[derive(Debug, Clone)]
pub struct AiService(Arc<dyn AiConnector>);

impl AiService {
    /// The service `connector` answers for.
    pub fn of(connector: impl AiConnector + 'static) -> Self {
        Self(Arc::new(connector))
    }

    /// Who answers, for the terminal alone.
    pub fn named(&self) -> String {
        self.0.named()
    }

    /// The answer to `question` from `sources`.
    pub fn answer(
        &self,
        pool: &mut Pool,
        identities: &mut Identities,
        question: &str,
        sources: &[Found],
    ) -> Asked {
        self.0.answer(pool, identities, question, sources)
    }

    /// The agent's next step for `asking`.
    ///
    /// # Errors
    ///
    /// As [`AiConnector::step`].
    pub fn step(
        &self,
        pool: &mut Pool,
        identities: &mut Identities,
        asking: &Asking<'_>,
    ) -> Result<agent_protocol::Read, String> {
        self.0.step(pool, identities, asking)
    }
}
