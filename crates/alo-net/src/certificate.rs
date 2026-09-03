//! When a certificate is not good enough, and what to tell somebody.
//!
//! **This is the part of TLS that is ours.** The handshake is rented and the
//! chain validation is rented; what no library can decide for us is what a
//! person is told when it fails, and that is the whole of the security value
//! at this layer.
//!
//! # Why it is a type and not a string
//!
//! Every other browser has arrived at the same place: a full-page interstitial
//! that says the connection is not private, and a button that goes on anyway.
//! People click the button. They click it because the page does not tell them
//! **what is wrong** or **what going on anyway would mean**, so the only
//! information they have is that they wanted to see the page.
//!
//! So a refusal here carries three things, and a caller cannot show one without
//! having the others: what is wrong, in a sentence; what trusting it anyway
//! would mean, in a sentence; and whether it is the kind of thing that could
//! ever be trusted at all.
//!
//! # Nothing here is bypassable
//!
//! There is no "accept anyway" in this crate, and no constructor that skips
//! verification. When a bypass exists it will be a deliberate, recorded act by
//! a person — queue item 127's security surfaces — and not a default, not a
//! flag, and not something a page can ask for.

use core::fmt;

/// What is wrong with a certificate.
///
/// Distinguished rather than flattened, because they are not the same problem
/// and do not deserve the same sentence. An expired certificate on a site
/// somebody trusts is usually an administrator's mistake; a name mismatch is
/// what an interception looks like.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fault {
    /// It was valid and no longer is.
    Expired,
    /// It is not valid yet — which is usually this machine's clock.
    NotYetValid,
    /// It is for a different host than the one asked for.
    WrongHost {
        /// The host that was asked for.
        asked_for: String,
    },
    /// Nothing this machine trusts signed it.
    UnknownIssuer,
    /// The chain does not hold together, or the signature does not check.
    BadSignature,
    /// Something the rented verifier refused for a reason of its own.
    ///
    /// Kept as its own kind rather than folded into one of the others: a
    /// verifier that grows a new check should surface it rather than have it
    /// mislabelled as an expiry.
    Other {
        /// What it said.
        detail: String,
    },
}

impl Fault {
    /// Whether a person could ever reasonably decide to go on anyway.
    ///
    /// **Not "is there a button"** — there is no button. It is whether the
    /// fault has an innocent explanation at all. A wrong host does not: it is
    /// what an interception looks like, and no amount of a person's confidence
    /// changes what the bytes say.
    pub fn could_ever_be_trusted(&self) -> bool {
        // The three with innocent explanations: an administrator forgot to
        // renew, this machine's clock is wrong, or an organisation runs its
        // own certificate authority and has not told this machine about it.
        // All three happen constantly and none of them is an attack.
        //
        // The other three have no innocent explanation. A wrong host is what
        // an interception looks like, and no amount of a person's confidence
        // changes what the bytes say.
        matches!(
            self,
            Fault::Expired | Fault::NotYetValid | Fault::UnknownIssuer
        )
    }

    /// What is wrong, in a sentence a person can act on.
    pub fn what_is_wrong(&self) -> String {
        match self {
            Fault::Expired => {
                "This site's certificate has expired. It was valid, and is not now.".to_owned()
            }
            Fault::NotYetValid => "This site's certificate is not valid yet. That usually means \
                 this machine's clock is wrong rather than that the site is."
                .to_owned(),
            Fault::WrongHost { asked_for } => format!(
                "This certificate is for a different site. It does not name {asked_for}, which is \
                 the address that was asked for."
            ),
            Fault::UnknownIssuer => "Nothing this machine trusts signed this site's certificate. \
                 Anybody can make one; what makes it mean anything is who signed it."
                .to_owned(),
            Fault::BadSignature => {
                "This site's certificate does not check out. The signatures on it do not hold \
                 together."
                    .to_owned()
            }
            Fault::Other { detail } => {
                format!("This site's certificate was refused: {detail}.")
            }
        }
    }

    /// What going on anyway would mean.
    ///
    /// The sentence every other browser leaves out, and the reason people
    /// click through: an interstitial that does not say what is being risked
    /// is asking somebody to decide with no information.
    pub fn what_trusting_it_means(&self) -> String {
        match self {
            Fault::Expired | Fault::NotYetValid => {
                "Going on would mean accepting a certificate nobody is standing behind any more. \
                 If it was replaced because it was stolen, this is exactly what that looks like."
                    .to_owned()
            }
            Fault::UnknownIssuer => "Going on would mean trusting whoever made this certificate, \
                 sight unseen — including with anything typed into this site."
                .to_owned(),
            Fault::WrongHost { .. } | Fault::BadSignature | Fault::Other { .. } => {
                "There is nothing to go on to. Whatever answered is not the site that was asked \
                 for, and anything sent to it would be sent to whoever that is."
                    .to_owned()
            }
        }
    }
}

/// A refusal, and everything a person needs to understand it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// The site that was being connected to.
    pub host: String,
    /// What is wrong with its certificate.
    pub fault: Fault,
}

impl Refused {
    /// What is wrong, in a sentence.
    pub fn what_is_wrong(&self) -> String {
        self.fault.what_is_wrong()
    }

    /// What trusting it anyway would mean, in a sentence.
    pub fn what_trusting_it_means(&self) -> String {
        self.fault.what_trusting_it_means()
    }

    /// Whether a person could ever reasonably decide to go on.
    ///
    /// There is no way to act on `true` in this crate, and that is deliberate:
    /// see the module's own note.
    pub fn could_ever_be_trusted(&self) -> bool {
        self.fault.could_ever_be_trusted()
    }
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} {}",
            self.host,
            self.what_is_wrong(),
            self.what_trusting_it_means(),
        )
    }
}

impl std::error::Error for Refused {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fault_says_what_is_wrong_and_what_trusting_it_would_mean() {
        // The point of the type: a caller cannot show one sentence without
        // having the other, so an interstitial cannot be built that leaves the
        // second one out.
        let faults = [
            Fault::Expired,
            Fault::NotYetValid,
            Fault::WrongHost {
                asked_for: "example.com".to_owned(),
            },
            Fault::UnknownIssuer,
            Fault::BadSignature,
            Fault::Other {
                detail: "something new".to_owned(),
            },
        ];
        for fault in faults {
            assert!(!fault.what_is_wrong().is_empty(), "{fault:?}");
            assert!(!fault.what_trusting_it_means().is_empty(), "{fault:?}");
            // Not jargon: a person reads these, so they end in a full stop and
            // do not contain the word "certificate authority" without saying
            // what one is.
            assert!(fault.what_is_wrong().ends_with('.'), "{fault:?}");
            assert!(fault.what_trusting_it_means().ends_with('.'), "{fault:?}");
        }
    }

    #[test]
    fn a_wrong_host_is_never_something_to_go_on_from() {
        // It is what an interception looks like. No amount of a person's
        // confidence changes what the bytes say.
        assert!(
            !Fault::WrongHost {
                asked_for: "bank.example".to_owned(),
            }
            .could_ever_be_trusted()
        );
        assert!(!Fault::BadSignature.could_ever_be_trusted());
    }

    #[test]
    fn the_faults_with_innocent_explanations_are_the_ones_that_have_them() {
        // An administrator forgot to renew; a machine's clock is wrong; an
        // organisation runs its own certificate authority. All three happen
        // constantly and none is an attack.
        assert!(Fault::Expired.could_ever_be_trusted());
        assert!(Fault::NotYetValid.could_ever_be_trusted());
        assert!(Fault::UnknownIssuer.could_ever_be_trusted());
    }

    #[test]
    fn a_wrong_host_says_which_host_was_asked_for() {
        let refused = Refused {
            host: "bank.example".to_owned(),
            fault: Fault::WrongHost {
                asked_for: "bank.example".to_owned(),
            },
        };
        assert!(refused.what_is_wrong().contains("bank.example"));
        assert!(refused.to_string().contains("bank.example"));
    }
}
