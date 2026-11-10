# ADR 0108 — Cookies are refused as a person chose, and a banner is answered with its refusal, by name

**Status:** accepted — the owner asked for it on 10 November 2026 (queue
item 920); built by 920 (see *Built*, below)
**Date:** 2026-11-10
**Context:** ADR 0002 (the layout tree is the agent's tree; no verb takes a
coordinate); ADR 0058 (Chromium is a renderer we drive, never script of
ours in its page); ADR 0060 (every request of a tab held and decided in
the browser process); ADR 0069 (each Chromium on a new profile of its
own, removed with it); ADR 0077 and ADR 0079 (alo carries its own Chromium,
the CEF host); ADR 0094 (trackers and adverts refused by lists alo
carries); ADR 0096 (alo's agent, and its verbs); ADR 0106 (how a
*Settings* section is added); ADR 0012 § 4 (a verb is an action in the
record); queue items 854/878/905 (*Settings*, kept in `settings.txt`).

## The question in one line

The owner, 10 November 2026: alo should answer a site's cookie banner
with its most private choice, and *Settings* should let a person refuse
cookies. How does a choice reach the Chromium alo drives, what is a banner
and what is its refusal, and what does alo never do?

## 1. Cookies: Chromium's own two preferences, in the new profile

Every Chromium alo drives — the machine's Chrome and alo's CEF host —
starts on a new, empty profile of its own (ADR 0069), so what it keeps of
cookies is what that profile says on its first start. alo writes, before
it starts, **`Default/Preferences`** holding Chromium's own preferences and
nothing else (`chrome_cookies.rs`):

| *Settings › Privacy › Cookies* | `profile.cookie_controls_mode` | `profile.default_content_setting_values.cookies` |
|---|---|---|
| **Refuse other sites' cookies** — the default | `1` | — |
| **Refuse all cookies** | `1` | `2` |
| **Keep all cookies** | `0` | — |

**What they are, in Chromium's source** (`main`, read 10 November 2026):

- `prefs::kCookieControlsMode = "profile.cookie_controls_mode"`
  (`components/content_settings/core/common/pref_names.h`); its values are
  `CookieControlsMode` in
  `components/content_settings/core/browser/cookie_settings.h` — `kOff =
  0` (*"Behaviorally equivalent to `kIncognitoOnly` as of June 2025"*),
  `kBlockThirdParty = 1`, `kIncognitoOnly = 2`, registered with
  `kIncognitoOnly` as its default (`cookie_settings.cc`).
  `CookieSettings::ShouldBlockThirdPartyCookiesInternal` returns true for
  `kBlockThirdParty`, and for the other two only in an incognito profile.
  Chromium's own enterprise policy `BlockThirdPartyCookies` sets the same
  preference to `kBlockThirdParty` (`cookie_settings_policy_handler.cc`).
- The cookies content setting is registered as `"cookies"` with
  `CONTENT_SETTING_ALLOW` (`content_settings_registry.cc`); a setting's
  default lives under `profile.default_content_setting_values.` and its
  name (`website_settings_info.cc`); `CONTENT_SETTING_BLOCK` is `2`
  (`components/content_settings/core/common/content_settings.h`). The same
  policy handler notes that a default of *block* blocks third-party
  cookies too.

**Measured**, the same day, with a server of the measurement's own setting
a first-party cookie on `127.0.0.1` and, from a frame on `localhost`
(another site), a `SameSite=None; Secure` one, each kept an hour, the page
loaded twice: Chrome 157.0.8103.50 headless with no file and with mode `0`
sent both back; with mode `1`, the page's own alone; with the default
`2`, neither. **A fresh Chromium profile with no preference keeps
another site's cookies** — which is why the default is written, not
assumed. The file is `Default/Preferences` for both Chromiums: CEF 156.0.14
is given the profile as `root_cache_path` and `cache_path` (the CEF host's
`host.rs`), and was measured to read the same file; a `Preferences` in the
directory itself was passed over by both (each kept the frame's cookie
under *Refuse other sites' cookies*). `tests/cookies_refused_as_chosen_through_chrome.rs`
holds all three choices through both.

**Why not the alternatives.**

- **CDP** has no command for the whole of it (`Network.pdl` and
  `Emulation.pdl` in `third_party/blink/public/devtools_protocol/domains/`,
  read the same day): `Network.setCookieControls` is `experimental`,
  third-party only — *"Sets Controls for third-party cookie access. Page
  reload is required before the new cookie behavior will be observed"* —
  and sent per session; `Emulation.setDocumentCookieDisabled`, also
  experimental, disables the `document.cookie` API and not a `Cookie`
  header.
- **Stripping headers at ADR 0060's gate** cannot reach the `Cookie`
  header Chromium adds after `Fetch` pauses a request, nor
  `document.cookie`.
- **A command-line switch**: none was found in Chromium's source that
  refuses every site's cookies; a switch would also have to be made for
  each choice, where the preferences are Chromium's own settings.
- **`CefRequestContext::SetPreference`** would change a live CEF host, but
  needs a message the host does not have, and is CEF's alone: the machine's
  Chrome has no such road. The file is one road for both.

**When it is used.** A Chromium keeps the choice it was started with for
its life. Each address opened from the conversation starts a Chromium of
its own (`conversation_chrome.rs`), so a choice is used **from the next
page alo opens**, which *Settings* says; a page already open keeps the
choice it opened with. *Queued* (923): the open card's Chromium told at
once.

**Partitioned cookies** (CHIPS, `Partitioned`) are kept by Chromium even
with another site's refused — keyed to the page they were set under, they
follow nobody across sites. Not measured here.

## 2. A banner and its refusal: by name, in the tree an agent reads

**A choice** is a button or a link, not disabled, whose accessible name,
written plainly (small letters, its spaces made one, curly apostrophes
straight, `&` as *and*, a final `.` or `!` dropped), is **exactly** one of
two lists (`consent_banner.rs`):

- **Refusals**, which alo may press: *Reject all*, *Reject*, *Reject
  optional*, *Reject non-essential*, *Decline*, *Decline all*, *Deny*,
  *Refuse*, *Refuse all*, *Disagree*, *Do not accept*, *Only necessary*,
  *Necessary only*, *Essential only*, *Use necessary cookies only*, *Allow
  necessary only*, *Accept only necessary* (OneTrust's), *Continue without
  accepting*, … with *cookies* after many; and the common platforms'
  standard labels in German (*Alle ablehnen*, *Ablehnen*, *Nur
  notwendige*), French (*Tout refuser*, *Refuser*, *Continuer sans
  accepter*), Spanish (*Rechazar todo*, *Rechazar*), Italian (*Rifiuta
  tutto*, *Rifiuta*), Dutch (*Alles weigeren*, *Weigeren*) and Portuguese
  (*Rejeitar tudo*, *Recusar*).
- **Consents**, which alo never presses: *Accept*, *Accept all*, *Allow
  all*, *Allow selection*, *Agree*, *I agree*, *OK*, *Got it*, … and their
  German, French, Spanish, Italian, Dutch and Portuguese labels.

A name is matched **whole**, never by a word in it: *Accept only
necessary* (a refusal) and *Accept all* (a consent) share their first
word, and no rule that read words could be trusted to keep them apart. The
two lists share no name, no refusal begins as *accept all*, *allow all*,
*agree* or *accept cookies* does, and both are tested so. Anything else —
*Adjust my preferences*, *More options*, *Customize*, *Close*, an × — is
never pressed. **A banner's second layer** (its preferences, where every
purpose would be switched off one by one) **is never opened**, and **a
close button is never taken for a refusal**: what closing a banner means
is the site's to say in words this does not read. A banner whose only
refusal is behind *More options*, or whose × is its refusal, is left to
the person (§ 3).

**A banner** is the nearest box around a choice, at most 8 boxes up, never
the page itself nor its `main`, whose words — its name and every name
under it — speak of cookies or consent (*cookie*, *consent*, *tracking
technolog…*, *personal data*, *your privacy*, and *Einwilligung*,
*Datenschutz*, *consentement*, *traceurs*, *consentimiento*, *consenso*,
*toestemming*, *consentimento*), and which holds at most 16 buttons and
links (a box holding more is most of a page that happens to say *cookie*
somewhere). A choice with no such box — an invitation's *Accept* and
*Decline*, a form's — is not a banner's, and is never pressed.

**Never by position, never by a site's markup.** The tree is the one an
agent reads (ADR 0002) — roles, names, states — so nothing here knows a
consent platform's ids or classes, which no tree holds; a platform that
renames its buttons in words this does not know is left to the person,
not guessed at.

**Measured on frozen copies**, taken on 10 November 2026 with headless
Chrome from the platforms' own sites (`crates/alo-window/tests/banners/`,
`SOURCES.txt`): the banner as Chrome's DOM held it after their scripts ran
— a few lines of the test's own beside it (which say what was pressed and
take the banner away, as a platform's script would), nothing of the
network left in it:

| Platform, page | Its choices, in order | alo |
|---|---|---|
| OneTrust (`cookiepro.com`) | *Allow All*, *Accept only necessary*, *Adjust my preferences* | presses *Accept only necessary* |
| Cookiebot (`cookiebot.com`) | *Deny*, *Customize*, *Allow all* | presses *Deny* |
| TrustArc (`trustarc.com`) | *Accept All*, *Reject Optional*, *Manage Settings* | presses *Reject Optional* |
| Quantcast Choice (`quantcast.com`) | *MORE OPTIONS*, *AGREE* | leaves it to the person |

**Not measured here, and queued** (924): no Didomi banner was shown to
this Mac on the pages tried (its script was on `bfmtv.com`, no notice
drawn — such banners are commonly shown only where the law asks for
consent); Google's Funding Choices was not met on any page tried;
Usercentrics' (on its own documentation site) was an empty `<aside
id="usercentrics-cmp-ui">` in what headless Chrome's `--dump-dom` wrote —
its banner drawn where that does not reach, a shadow root by all
appearances — so it could not be frozen this way; Sourcepoint's (on `welt.de`) is a frame from
another site, which the tree alo reads of a tab may not hold — not
measured; and the lists' other languages were not met on a live page.
Each is a frozen page to take from where it is shown.

## 3. alo never presses a consent; a banner with no refusal is the person's

alo presses **a refusal or nothing**. A banner offering only consents —
*AGREE* and *MORE OPTIONS* — is **left to the person**, and the card's bar
says so (§ 4). So is one whose refusal could not be pressed (the verb
refused it, something stood over it): it is said, with why, and not tried
again on that document.

**A page that hides its content behind consent** — a cookie wall, a *pay
or accept* — is left as it is: if it offers a refusal, the refusal is
pressed and whatever the site then shows is the site's; if it offers only
consent, the person sees the card's line and chooses. alo never accepts
to reveal a page, and never removes a banner or its veil (that would be
script or a style sheet of ours deciding what a site shows; ADR 0094's
amendment hides an advert's empty box by the lists alone).

## 4. How it is done, and how it is shown

- **When**: each document of a card's page, once it has loaded, and —
  since platforms draw their banner a moment after `load` — again 1, 2 and
  4 seconds after a look that found nothing: four looks at most. A banner
  found, answered or left, is the end of it for that document; the page
  going elsewhere, or loaded again, is a new document
  (`consent_answer.rs`). It is done by the thread holding the card's tab,
  between its other work, as the shield's count is.
- **How**: the page read as the tree an agent reads, the refusal found by
  name, and pressed by **the agent's own verb** — `activate` on the node it
  was read as (`Target::Node`) — decided by `alo-agent` against the page
  as it is then and carried out as a person's click at the middle of that
  node after Chromium is asked what a press there would press (ADR 0058 §
  4). No coordinate of ours, no script in the page. The press is an action
  in the tab's record like any verb (ADR 0012 § 4).
- **Shown**: quietly in the card's bar, after the address and before the
  shield, as *Loading* is drawn — **Cookies refused**, or **Cookie banner
  left to you** — and on the terminal in full, the page's own words quoted:
  *alo answered the card's page's cookie banner with its most private
  choice, "Reject all" (ADR 0108)*. Read by an agent as a paragraph of the
  bar. Never a platform's name (`no_providers_name_on_screen.rs`'s rule):
  the bar's words are fixed, and the button's own words go to the terminal
  only.
- **Turned off**: *Settings › Privacy › Cookie banners › Leave them to me*
  looks at nothing; used from the next page loaded.

## 5. *Settings › Privacy*

A fourth section under *Search and answers*, built as ADR 0106 built its
(`settings_privacy.rs`, `settings_privacy_document.rs`; the window's
`conversation_settings.rs` and `settings_file.rs`): two radio groups,
**Cookies** (*Refuse other sites' cookies* · Default, *Refuse all cookies*
— under it, *Signing in to sites will fail: a site cannot remember you.* —
*Keep all cookies*) and **Cookie banners** (*Answer with the most private
choice* · Default, *Leave them to me*). Tab reaches each after *Search and
answers*; the arrows choose round; a press chooses. Kept in `settings.txt`
as `cookies all|keep` and `banners leave`, each only when not the default,
and told to the whole browser process at once (`privacy_chosen.rs`); set
by the window alone — no page, renderer or agent can.

## Alternatives rejected

- **A consent platform's own API** (`__tcfapi`, OneTrust's
  `OneTrust.RejectAll()`): script of ours in a stranger's page (ADR 0058),
  and a different road for each platform.
- **A list of platforms' selectors**, as some blockers carry: markup no
  tree an agent reads holds, which a site changes without notice; ADR
  0002 reads what a person reads.
- **Hiding the banner** with a cosmetic rule: the site has not been
  answered, and a cookie wall stays a wall underneath.
- **Pressing a close button**: a close is consent on some sites and
  nothing on others; only words can say which (§ 2).
- **Pressing a consent to reveal a page**: never.

## Built (10 November 2026)

Queue item 920: `chrome_cookies.rs` (the preferences file, written by
`chrome.rs` as every Chromium starts), `privacy_chosen.rs`,
`consent_banner.rs`, `consent_answer.rs` (run from `chrome_card.rs`'s
pass), the card's `Consent` and its bar's line, *Settings › Privacy*, the
file's two lines. Cut, and queued: 923 (a cookie choice used by the open
card at once), 924 (the platforms and languages not met here, and a
cross-site frame's banner). Not verified by hand on live sites: see the
journal.
