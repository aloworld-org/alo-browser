/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The alo browser's window: the browser process a person starts.
//!
//! ADR 0024. Its binary is `alo`. It holds the window and nothing of a page
//! (ADR 0005): it owns the tabs, which own the renderers, and it shows what
//! they paint.
//!
//! # How it is put together
//!
//! - [`window`] is the event loop, the one file that names `winit`, and
//!   [`present`] puts pixels in the window, the one file that names
//!   `softbuffer` (ADR 0024 § 1).
//! - [`conductor`] is the thread that owns the tabs and is the only thing
//!   that ever waits on a renderer; it and the event loop speak only in
//!   [`message`]s (§ 2). It waits on its [`inbox`] and never on a server:
//!   [`network`] is the thread that makes every request (ADR 0041), and
//!   [`hold`] keeps a load's first frame back for its style sheets.
//! - [`showing`] is what the window was last sent, and [`compose`] makes the
//!   window's pixels from it — a function, tested by reference render, placed
//!   by [`place`] at the window's [`scale`] and saying what happened to a tab
//!   with [`notice`]. [`mark`] rings the box an agent's verb acted on, on
//!   the frame painted after it (item 624).
//! - [`keys`] is what a person's key asks of the tabs, when it is one of the
//!   browser's own (item 529), read by the event loop before anything else.
//! - [`opening`] is what a person named on the command line, [`fonts`] the
//!   fonts renderers start with, and [`beside`] where the renderer program is.
//!   Inside the macOS application, [`bundle`] decides what is opened with
//!   nothing named (item 477), and [`bundle_chromium`] that only the
//!   Chromium the application carries is started there, never the machine's
//!   Chrome (item 719, ADR 0077), and [`bundle_refused`] that a hand run's
//!   `--sign-ins` is refused there (item 973, ADR 0111 § 6); `alo-bundle` makes that application, put
//!   together by [`bundle_made`] with the [`info_plist`] it says of itself
//!   and sealed by [`bundle_seal`] (item 687). The Chromium it carries
//!   is CEF's, loaded by alo's own host program (`crates/alo-cef-host`, ADR
//!   0079), laid out as an application of its own by [`cef_host`] (item
//!   725), checked against its pin by [`cef_pin`] and put in the
//!   application with its licences by [`bundle_carried`] (item 720).
//! - [`agents`] is the socket an agent outside the browser speaks to (ADR
//!   0055), at a place [`socket_place`] makes private; [`protocol`] reads and
//!   writes its lines, one of the two files that name `serde_json`, into what
//!   [`asking`] says they mean; [`serving`] answers them from the tabs on the
//!   conductor's thread, kept to the sites a person named in
//!   [`agent_sites`] — and, for `print_pdf`, to the directories a person
//!   allowed in [`agent_saves`], the two held as one [`agent_scope`] — and
//!   [`agent_record`] is what it records; `find` is answered by
//!   [`agent_find`], by name and never by place (item 956), and
//!   `page_text` by [`agent_text`], as words with no place (item 964); a second
//!   `alo` started on the same profile hands its address to the running
//!   one over it, by [`launch_handover`] (item 976, ADR 0114). The socket
//!   is a Unix one, and Windows is undecided (ADR 0055 § 1). What a verb
//!   that ran was answered with is shown to the person watching, in a line
//!   of the window's own ([`message::News::AgentSaid`], item 623), and so is
//!   why one was refused ([`message::News::AgentRefused`], item 625), for as
//!   long as [`agent_lines`] says.
//! - [`chrome`] is the other renderer, the machine's own Chrome, started and
//!   spoken to over a pipe (ADR 0058, item 619): [`chrome_wire`] finds where
//!   each of its messages ends and [`chrome_protocol`] reads them, the one
//!   other file that names `serde_json`, with [`json_depth`] refusing any
//!   JSON from outside that nests too deep before either reader sees it;
//!   what is read and not yet taken is held by [`chrome_held`], bounded in
//!   messages and in bytes (item 653). Each Chrome starts on a profile of its own that [`chrome_profile`]
//!   makes, keeping it on the machine's resolver (ADR 0008, item 635) and
//!   locked while its browser lives, and removing as it is made the
//!   profiles killed browsers left, which nobody holds (ADR 0069, item 690). A
//!   page in it is a [`chrome_tab`], read as the tree an agent reads by
//!   [`chrome_tree`] (item 620), and acted on by [`chrome_act`]: each verb
//!   decided by `alo-agent` against a [`chrome_reading`] of it, and carried
//!   out as a real click or real typing (item 621), a date a segment at a
//!   time as [`chrome_date`] reads them (item 632), an option chosen in a
//!   list Chrome draws by [`chrome_choose`] (item 660); a form Chrome would not
//!   send is known by the bubble [`chrome_bubble`] reads (item 631) and by
//!   each control Chrome logs it could not focus, which
//!   [`chrome_unfocusable`] reads (item 646), and
//!   what the page's script said while a verb ran is put into the engine's
//!   words by [`chrome_said`] (item 639). A page from the network
//!   is carried by Chrome's own network (ADR 0060): every target of its tab
//!   held by [`chrome_hold`] until its requests are paused, and each request
//!   judged by [`chrome_gate`] before Chrome is let send it (item 633), and
//!   written into the record with the cause this process gave it by
//!   [`chrome_record`] (item 634) — a request paused as its frame goes to a
//!   new document owed by [`chrome_owed`] until Chrome names the document
//!   that asked (item 647). A Chrome started shown draws its one
//!   page in a window of its own, which [`chrome_window`] sizes around the
//!   page and places on the screen (ADR 0061, item 627), and a card's window
//!   is parked by [`chrome_park`] by a display at the conversation's scale
//!   (ADR 0090 as amended, item 772); where a frame of
//!   ours stands around it is [`chrome_frame`]'s to say, from Chrome's word
//!   and what [`window_server`] shows on the screen (ADR 0062, item 672),
//!   and what that frame's window is told as Chrome's moves, hides and
//!   closes is [`chrome_follow`]'s (item 642). Everything the frame is told
//!   is a [`frame_told::ToFrame`] — where to stand, an agent's line, or
//!   what Chrome calls the page, which [`chrome_named`] reads (item 681) —
//!   and what it draws, placed by [`frame_place`], is the page's tab, drawn
//!   by a tab strip's renderer of its own that [`frame_strip`] holds (item
//!   681), which renderer draws the page, and those lines (item 679). The
//!   box an agent's verb acted on is marked over Chrome's page by Chrome
//!   itself, as [`chrome_mark`] asks it to (item 680, ADR 0066).
//! - [`pages`] is what [`serving`] asks of the tabs, whichever renderer
//!   draws them: [`engine_pages`] answers it from this engine's, and
//!   [`chrome_pages`] from a tab in Chrome, which [`chrome_serving`] holds
//!   on a thread of its own and answers the agent socket about, through the
//!   same serving and the same grants (item 630).
//! - [`conversation`] is the browser's own interface as a conversation
//!   (ADR 0074, item 709), what `alo --conversation` opens: the chats
//!   panel and the first screen, each drawn by a confined renderer of its
//!   own that [`conversation_drawing`] holds, placed by
//!   [`conversation_place`] and composed by [`conversation_compose`]. A
//!   person's press on either is aimed by [`conversation_aim`], claimed by
//!   the part's renderer and decided by the browser process (item 710).
//!   The one box on the first screen is [`conversation_box`]: a person's
//!   keys, read by [`typing`], edit what it holds, and Return decides to
//!   go to an address typed (item 701), which [`conversation_chrome`]
//!   opens in the machine's Chrome, in a window of its own beside no frame
//!   of ours (item 713, ADR 0075). The panel's *Search chats* is
//!   [`conversation_find`], edited by the same keys while it has the
//!   keyboard (item 711). Whether the panel stands whole or folded to its
//!   rail — hidden by a person, or the window narrower than 760 — is
//!   [`conversation_fold`], with the least window (item 775); there the
//!   whole panel slides over the conversation as a sheet
//!   ([`conversation_sheet`], item 776). The conversation's bar fades once
//!   it stops scrolling ([`conversation_bar`], item 793).
//! - [`accessible`] puts the conversation's window on the platform's
//!   accessibility bus (ADR 0115 phase (a), item 987), the one file that
//!   names `accesskit_winit`; [`accessible_tree`] maps the parts' trees to
//!   AccessKit's and what the bus asks back to [`assisted`]'s words, which
//!   say what each node is offered and what is never published, and
//!   [`assisted_record`] is what assistive technology did. A card's page
//!   is grafted under the card's picture of it (phase (b), item 988), read
//!   and acted on through the agent socket's own orders by
//!   [`assisted_page`].
//!
//! # What it is not yet
//!
//! One window. The tab strip is drawn by a renderer of its own (`alo-renderer`'s
//! `strip`, item 297) and shown across the top of the window, the selected
//! tab below it (item 528), and a person opens, closes and chooses tabs with
//! the browser's own keys (item 529), and presses the page with the main
//! mouse button and moves over it ([`aim`] decides what a point is over,
//! [`cursor`] what the page is sent, items 535 and 542) or the strip to
//! choose a tab (item 539). The page and the strip are painted at the
//! window's [`scale`] (item 562, ADR 0057 § 3), 1.5 among them (item 599).
//! Not yet: an agent reading the strip (item 531), a scale factor nobody
//! has measured, such as 1.25 — painted at the largest measured scale below
//! it — and a link opening a tab (item 300).

pub mod accessible;
pub mod accessible_tree;
pub mod acting_mark;
pub mod agent_changes;
pub mod agent_dropdown;
pub mod agent_find;
pub mod agent_found;
pub mod agent_gate;
pub mod agent_kept;
pub mod agent_lines;
pub mod agent_opening;
pub mod agent_page;
pub mod agent_paying;
pub mod agent_protocol;
pub mod agent_record;
pub mod agent_saves;
pub mod agent_scope;
pub mod agent_signin;
pub mod agent_sites;
pub mod agent_text;
#[cfg(unix)]
pub mod agents;
pub mod ai_service;
pub mod aim;
pub mod answer_key;
pub mod answer_protocol;
pub mod app_views;
pub mod app_ways;
pub mod asking;
pub mod assisted;
pub mod assisted_page;
pub mod assisted_record;
pub mod beside;
pub mod blocking;
pub mod blocking_off;
pub mod brave_search;
pub mod bundle;
pub mod bundle_carried;
pub mod bundle_chromium;
pub mod bundle_icon;
pub mod bundle_keys;
pub mod bundle_made;
pub mod bundle_refused;
pub mod bundle_seal;
pub mod card_aim;
pub mod card_clicks;
pub mod card_cursor;
pub mod card_input;
pub mod card_keys;
pub mod card_moves;
pub mod card_pointer;
pub mod carried_keys;
pub mod cef_host;
pub mod cef_pin;
pub mod chat_delete;
pub mod chat_key;
pub mod chat_name;
pub mod chat_record;
pub mod chat_seal;
pub mod chat_store;
#[cfg(unix)]
pub mod chrome;
pub mod chrome_act;
pub mod chrome_bubble;
pub mod chrome_card;
pub mod chrome_card_caret;
pub mod chrome_card_input;
pub mod chrome_card_list;
pub mod chrome_card_title;
pub mod chrome_cast_wire;
pub mod chrome_choose;
pub mod chrome_cookies;
pub mod chrome_cosmetic;
pub mod chrome_date;
pub mod chrome_follow;
pub mod chrome_frame;
pub mod chrome_gate;
pub mod chrome_held;
pub mod chrome_history;
pub mod chrome_hit;
pub mod chrome_hold;
pub mod chrome_iframes;
pub mod chrome_loading;
pub mod chrome_mark;
pub mod chrome_menu;
pub mod chrome_named;
pub mod chrome_owed;
pub mod chrome_pages;
pub mod chrome_park;
pub mod chrome_pin;
pub mod chrome_profile;
pub mod chrome_protocol;
pub mod chrome_reading;
pub mod chrome_record;
pub mod chrome_said;
pub mod chrome_screencast;
pub mod chrome_serving;
pub mod chrome_sign_ins;
pub mod chrome_tab;
pub mod chrome_tell;
pub mod chrome_told;
pub mod chrome_tree;
pub mod chrome_type_ahead;
pub mod chrome_unfocusable;
pub mod chrome_window;
pub mod chrome_wire;
pub mod clipboard;
pub mod closing;
pub mod colours;
pub mod compose;
pub mod conductor;
pub mod consent_answer;
pub mod consent_banner;
pub mod conversation;
pub mod conversation_agent;
pub mod conversation_agents;
pub mod conversation_aim;
pub mod conversation_answer;
pub mod conversation_bar;
pub mod conversation_bar_press;
pub mod conversation_box;
pub mod conversation_chrome;
pub mod conversation_compose;
pub mod conversation_composed;
pub mod conversation_drawing;
pub mod conversation_find;
pub mod conversation_fold;
pub mod conversation_glide;
pub mod conversation_keys;
pub mod conversation_picture;
pub mod conversation_place;
pub mod conversation_search;
pub mod conversation_services;
pub mod conversation_settings;
pub mod conversation_sheet;
pub mod conversation_sign_ins;
pub mod conversation_thread;
pub mod conversation_window;
pub mod conversation_work;
pub mod cursor;
pub mod data_dir;
pub mod engine_pages;
pub mod field_blink;
pub mod field_edit;
pub mod fonts;
pub mod frame_place;
pub mod frame_strip;
pub mod frame_told;
pub mod hold;
pub mod icns;
pub mod inbox;
pub mod info_plist;
pub mod json_depth;
pub mod kept_cookie;
pub mod key_bindings;
pub mod keychain;
pub mod keys;
#[cfg(unix)]
pub mod launch_handover;
pub mod mark;
pub mod message;
pub mod network;
pub mod notice;
pub mod openai_answer;
pub mod opening;
pub mod pages;
pub mod place;
pub mod present;
pub mod present_chosen;
// The one `unsafe` boundary of the workspace (ADR 0105): `wgpu`'s Metal
// layer tagged with a colour space, in one block. `scripts/gate.sh` checks
// that the lint is lifted here alone, and `unsafe` written in that file alone.
#[cfg(target_os = "macos")]
#[expect(
    unsafe_code,
    reason = "ADR 0105: the Metal layer wgpu made, tagged as softbuffer tags its images"
)]
mod present_kept;
pub mod present_shown;
pub mod privacy_chosen;
pub mod protocol;
pub mod scale;
pub mod scrolled;
pub mod search_key;
pub mod search_marks;
pub mod search_protocol;
pub mod search_service;
pub mod search_text;
pub mod search_thumbnails;
pub mod searxng_protocol;
pub mod searxng_search;
pub mod service_keys;
pub mod services;
pub mod services_made;
pub mod serving;
pub mod settings_file;
pub mod settings_keys;
pub mod showing;
pub mod sign_in_key;
pub mod sign_ins;
#[cfg(unix)]
pub mod socket_place;
pub mod socket_plain;
pub mod text_field;
pub mod text_stops;
pub mod text_undo;
pub mod typing;
pub mod window;
pub mod window_server;
pub mod window_title;
