/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Everything a person kept agents outside this browser to, as one value
//! handed to whatever answers them ([`crate::serving`]): the sites they may
//! reach ([`crate::agent_sites`], queue item 497) and the directories a page
//! may be saved in for them ([`crate::agent_saves`], queue item 956).
//!
//! Both are a person's, named as the browser starts, and both only narrow
//! (ADR 0055 § 8): nothing an agent sends can name, widen or read either.

use crate::agent_saves::AgentSaves;
use crate::agent_sites::AgentSites;

/// What agents outside this browser are kept to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentScope {
    /// The sites they may reach.
    pub sites: AgentSites,
    /// Where a page may be saved for them.
    pub saves: AgentSaves,
}

impl AgentScope {
    /// Kept to `sites`, with a page saved in `saves`.
    pub fn new(sites: AgentSites, saves: AgentSaves) -> Self {
        Self { sites, saves }
    }
}

impl From<AgentSites> for AgentScope {
    /// Kept to `sites`, with nowhere a page may be saved.
    fn from(sites: AgentSites) -> Self {
        Self {
            sites,
            saves: AgentSaves::none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sites_alone_save_nowhere() {
        let scope = AgentScope::from(AgentSites::Every);
        assert_eq!(scope.sites, AgentSites::Every);
        assert!(scope.saves.is_empty());
    }
}
