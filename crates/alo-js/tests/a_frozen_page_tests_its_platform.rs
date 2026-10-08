/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `alo-downloads`, alo's public download page: the first frozen script that
//! runs to reach a regular expression, and the page queue item 74 closes on.
//!
//! Before item 74 the whole script was refused at byte 144 — `/Mac/.test(p)`,
//! "a regular expression literal is not built yet". Now the whole of it
//! compiles, and its first five lines — reading the platform, and four
//! patterns that decide which card is the visitor's — give the answers a
//! browser gives.
//!
//! The page reads `navigator.platform` and `navigator.userAgent`, and no
//! `Navigator` is built yet (queue item 325), so this runs those lines under
//! a stand-in `navigator` declared in front of them. The page's own bytes are
//! not edited: the stand-in is a separate statement, and the lines are cut
//! from the frozen file where its sixth line begins.

use std::path::{Path, PathBuf};

use alo_js::interpret::{Engine, Trouble};
use alo_js::object::Value;
use alo_js::{compile, script};

/// The frozen page.
fn page() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("alo-corpus")
        .join("cases")
        .join("alo-downloads")
        .join("page.html")
}

/// The page's one script, exactly as frozen, or a sentence saying why it
/// could not be found, which the test turns into its failure.
fn the_script() -> Result<String, String> {
    let html = std::fs::read_to_string(page())
        .map_err(|error| format!("the frozen page could not be read: {error}"))?;
    let (_, after) = html
        .split_once("<script>")
        .ok_or("the page has no inline script")?;
    let (script, _) = after
        .split_once("</script>")
        .ok_or("the page's script is not closed")?;
    Ok(script.to_owned())
}

/// The script up to the line that begins `if (isMac)`, which is where it
/// starts writing to the document.
fn the_platform_lines() -> Result<String, String> {
    let script = the_script()?;
    let (lines, _) = script
        .split_once("if (isMac)")
        .ok_or("the script no longer decides by isMac")?;
    Ok(lines.to_owned())
}

/// Run the platform lines under a stand-in `navigator`, both ordinarily and
/// with the collector at every allocation, and answer `isMac,isWin` from
/// each run.
fn platform(platform: &str, agent: &str) -> Result<[String; 2], String> {
    let source = format!(
        "var navigator = {{ platform: \"{platform}\", userAgent: \"{agent}\" }};\n{}\nisMac + \",\" + isWin",
        the_platform_lines()?
    );
    let program =
        script(&source).map_err(|why| format!("the platform lines did not parse: {why}"))?;
    let run = |stress: bool| -> Result<String, String> {
        let mut engine = Engine::new().map_err(|why| format!("no engine: {why}"))?;
        engine.objects().heap_mut().stress(stress);
        Ok(match engine.evaluate(&program) {
            Ok(Value::Text(held)) => engine
                .objects()
                .units(held)
                .map(String::from_utf16_lossy)
                .unwrap_or_default(),
            Ok(other) => format!("{other:?}"),
            Err(Trouble::Escaped(escape)) => format!("! {escape}"),
            Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
        })
    };
    Ok([run(false)?, run(true)?])
}

/// Check one platform: both runs agree, and they answer `expected`.
fn decides(name: &str, agent: &str, expected: &str) -> Result<(), String> {
    let [ordinary, stressed] = platform(name, agent)?;
    if ordinary != stressed {
        return Err(format!(
            "{name}: {ordinary} ordinarily and {stressed} with the collector at every allocation"
        ));
    }
    if ordinary != expected {
        return Err(format!("{name}: {ordinary}, not {expected}"));
    }
    Ok(())
}

#[test]
fn the_whole_script_compiles_past_its_first_regular_expression() {
    let script_text = match the_script() {
        Ok(text) => text,
        Err(why) => panic!("{why}"),
    };
    assert!(
        script_text.contains("/Mac/.test(p)"),
        "the line that was refused is still in the page"
    );
    let Ok(program) = script(&script_text) else {
        panic!("the page's script parses");
    };
    assert!(
        compile(&program).is_ok(),
        "and compiles, four patterns and all"
    );
}

#[test]
fn the_pages_own_patterns_tell_a_mac_from_windows() {
    for (name, agent, expected) in [
        (
            "MacIntel",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15",
            "true,false",
        ),
        (
            "Win32",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "false,true",
        ),
        (
            "Linux x86_64",
            "Mozilla/5.0 (X11; Linux x86_64)",
            "false,false",
        ),
        // An empty platform, which the page's own `|| ""` allows for, still
        // reaches the user agent's pattern.
        ("", "Mozilla/5.0 (Macintosh; Mac OS X 14_0)", "true,false"),
    ] {
        if let Err(why) = decides(name, agent, expected) {
            panic!("{why}");
        }
    }
}
