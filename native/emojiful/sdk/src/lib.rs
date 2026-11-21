//! emojiful: Emojiful 4.2.0 (HorizonStudio, MIT), EmojiFontRenderer.getEmojiFormattedString for chat
//! lines. Each `:short_name:` of a Twemoji emoji becomes that emoji's glyph character in this
//! package's bitmap font `emojiful:emoji` (assets/, tools/emojiful_port.py with this table). Emojiful draws the image
//! untinted and without a shadow: the glyph's span is white with shadow colour 0. Only the line
//! shown changes; the narrator still reads the message.
use mod_api::text::TextContent;
use mod_api::{ClientChatReceivedEvent, ClientHost, Flow, Hooks, Text};

/// The glyphs' font (assets/emojiful/font/emoji.json): a mod can't add to `minecraft:default`.
const FONT: &str = "emojiful:emoji";

mod table;
use table::EMOJI;

fn lookup(name: &str) -> Option<char> {
    EMOJI.binary_search_by(|(n, _)| (*n).cmp(name)).ok().map(|i| EMOJI[i].1)
}

fn name_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '+')
}

/// `text` cut into plain runs and emoji glyphs; None when it holds no known `:name:`.
fn segments(text: &str) -> Option<Vec<(String, bool)>> {
    if !text.contains(':') {
        return None;
    }
    let mut out: Vec<(String, bool)> = Vec::new();
    let mut plain = String::new();
    let mut found = false;
    let mut rest = text;
    while let Some(i) = rest.find(':') {
        plain.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let end = after.find(|c: char| !name_char(c));
        let emoji = match end {
            Some(e) if e > 0 && after[e..].starts_with(':') => lookup(&after[..e]).map(|c| (c, e)),
            _ => None,
        };
        match emoji {
            Some((c, e)) => {
                if !plain.is_empty() {
                    out.push((std::mem::take(&mut plain), false));
                }
                out.push((c.to_string(), true));
                found = true;
                rest = &after[e + 1..];
            }
            None => {
                plain.push(':');
                rest = after;
            }
        }
    }
    plain.push_str(rest);
    if !plain.is_empty() {
        out.push((plain, false));
    }
    found.then_some(out)
}

/// Rewrites every literal in `text` (its children and translation arguments too); true when one
/// held an emoji code. A literal with codes becomes an empty literal whose first children are
/// its runs, so the order and the inherited style stay.
fn convert(text: &mut Text) -> bool {
    let mut found = false;
    match &mut text.content {
        TextContent::Literal(s) => {
            if let Some(parts) = segments(s) {
                found = true;
                s.clear();
                let runs: Vec<Text> = parts
                    .into_iter()
                    .map(|(run, emoji)| {
                        let mut t = Text::literal(run);
                        if emoji {
                            t.style.color = Some("#FFFFFF".into());
                            t.style.shadow_color = Some(0);
                            t.style.obfuscated = Some(false);
                            t.style.font = Some(FONT.into());
                        }
                        t
                    })
                    .collect();
                text.children.splice(0..0, runs);
            }
        }
        TextContent::Translatable { arguments, .. } => {
            for a in arguments {
                found |= convert(a);
            }
        }
        _ => {}
    }
    for child in &mut text.children {
        found |= convert(child);
    }
    found
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn client_chat_received(&self, _host: &mut dyn ClientHost, event: &mut ClientChatReceivedEvent<'_>) -> Flow {
        if event.overlay {
            return Flow::Continue;
        }
        let mut line = event.message.clone();
        if convert(&mut line) {
            *event.message = line;
        }
        Flow::Continue
    }
}
