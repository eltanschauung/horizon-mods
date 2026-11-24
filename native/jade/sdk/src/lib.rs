use mod_api::*;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub const REQUEST: &str = "jade:request";
pub const DATA: &str = "jade:data";

const KEYS: [[&str; 3]; 11] = [
    ["config", "Open Config", "key.keyboard.keypad.0"],
    ["show_overlay", "Show Overlay", "key.keyboard.keypad.1"],
    ["toggle_liquid", "Toggle Fluid", "key.keyboard.keypad.2"],
    ["show_recipes", "Show Recipes", "key.keyboard.keypad.3"],
    ["show_uses", "Show Uses", "key.keyboard.keypad.4"],
    ["narrate", "Narrate Target", "key.keyboard.keypad.5"],
    ["show_details", "Show Details", "key.keyboard.left.shift"],
    ["profile.0", "Use Profile #0", "key.keyboard.unknown"],
    ["profile.1", "Use Profile #1", "key.keyboard.unknown"],
    ["profile.2", "Use Profile #2", "key.keyboard.unknown"],
    ["profile.3", "Use Profile #3", "key.keyboard.unknown"],
];

type Message = (PlayerRef, Vec<u8>);

thread_local! {
    static REQUESTS: RefCell<VecDeque<Message>> = const { RefCell::new(VecDeque::new()) };
    static REPLIES: RefCell<VecDeque<Vec<u8>>> = const { RefCell::new(VecDeque::new()) };
}
static OPEN_CONFIG: AtomicBool = AtomicBool::new(false);

pub fn take_requests() -> VecDeque<Message> {
    REQUESTS.with_borrow_mut(std::mem::take)
}
pub fn take_replies() -> VecDeque<Vec<u8>> {
    REPLIES.with_borrow_mut(std::mem::take)
}
pub fn open_config() -> bool {
    OPEN_CONFIG.swap(false, Ordering::Relaxed)
}

struct RequestHandler;
impl IPayloadHandler for RequestHandler {
    fn handle(&self, bytes: PayloadBytes, ctx: &mut dyn IPayloadContext) -> Result<(), PayloadHandleError> {
        if bytes.0.len() <= 4096 && let Ok(player) = ctx.player() {
            REQUESTS.with_borrow_mut(|queue| {
                if queue.len() < 256 && queue.iter().filter(|(owner, _)| *owner == player).count() < 12 {
                    queue.push_back((player, bytes.0));
                }
            });
        }
        Ok(())
    }
}

struct ReplyHandler;
impl IPayloadHandler for ReplyHandler {
    fn handle(&self, bytes: PayloadBytes, _: &mut dyn IPayloadContext) -> Result<(), PayloadHandleError> {
        if bytes.0.len() <= 128 * 1024 {
            REPLIES.with_borrow_mut(|queue| {
                if queue.len() == 16 { queue.pop_front(); }
                queue.push_back(bytes.0);
            });
        }
        Ok(())
    }
}

#[derive(Default)]
struct Jade;
#[mod_api::horizon_mod]
impl Hooks for Jade {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for [key, label, default] in KEYS {
            r.register_key_mapping(KeyRegistration::new(key, default).label(label));
        }
        let _ = r.register_setting(SettingSpec::toggle("f3_only", "F3 Only", true));
        let _ = r.register_setting(SettingSpec::toggle("open_jade_config", "Open Jade settings", false));
        r.payload_handlers().registrar("1").optional()
            .play_to_server(PayloadId(REQUEST.into()), Arc::new(RequestHandler))
            .play_to_client(PayloadId(DATA.into()), Some(Arc::new(ReplyHandler)));
    }

    fn register_client_commands(&self, _: &mut dyn ClientHost, event: &mut RegisterClientCommandsEvent<'_>) {
        for usage in [
            "jadec config",
            "jadec pin",
            "jadec use_profile <profile:integer>",
            "jadec handlers",
            "jadec generate_server_overrides [pattern:text]",
        ] {
            event.commands.push(Command::new(usage));
        }
    }

    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        for usage in ["jade show <targets:player>", "jade hide <targets:player>"] {
            event.commands.push(Command::op(usage, 2));
        }
    }

    fn setting_changed(&self, host: &mut dyn ClientHost, event: &mut SettingChangedEvent<'_>) {
        if event.mod_id == "jade" && event.key == "open_jade_config"
            && host.setting("jade", event.key).as_deref() == Some("true")
        {
            host.set_setting("jade", event.key, "false");
            OPEN_CONFIG.store(true, Ordering::Relaxed);
        }
    }

    fn player_logged_out(&self, _: &mut dyn ServerHost, event: &mut PlayerLoggedOutEvent<'_>) {
        REQUESTS.with_borrow_mut(|queue| queue.retain(|(player, _)| player.uuid() != event.player));
    }

    fn client_disconnecting(&self, _: &dyn ClientHost, _: &mut ClientDisconnectingEvent) {
        REPLIES.with_borrow_mut(|queue| queue.clear());
    }

    fn server_stopping(&self, _: &mut dyn ServerHost, _: &mut ServerStoppingEvent) {
        REQUESTS.with_borrow_mut(|queue| queue.clear());
    }
}

/// `assets/jade/lang/<code>.json` of the jar (this bundle's `data/lang`), compiled in.
pub fn jar_lang(code: &str) -> Option<&'static [u8]> {
    let bytes: &'static [u8] = match code {
        "de_de" => include_bytes!("../assets/jade/lang/de_de.json"),
        "en_us" => include_bytes!("../assets/jade/lang/en_us.json"),
        "es_es" => include_bytes!("../assets/jade/lang/es_es.json"),
        "fr_fr" => include_bytes!("../assets/jade/lang/fr_fr.json"),
        "it_it" => include_bytes!("../assets/jade/lang/it_it.json"),
        "ja_jp" => include_bytes!("../assets/jade/lang/ja_jp.json"),
        "ko_kr" => include_bytes!("../assets/jade/lang/ko_kr.json"),
        "nl_nl" => include_bytes!("../assets/jade/lang/nl_nl.json"),
        "pl_pl" => include_bytes!("../assets/jade/lang/pl_pl.json"),
        "pt_br" => include_bytes!("../assets/jade/lang/pt_br.json"),
        "ru_ru" => include_bytes!("../assets/jade/lang/ru_ru.json"),
        "tr_tr" => include_bytes!("../assets/jade/lang/tr_tr.json"),
        "uk_ua" => include_bytes!("../assets/jade/lang/uk_ua.json"),
        "zh_cn" => include_bytes!("../assets/jade/lang/zh_cn.json"),
        "zh_tw" => include_bytes!("../assets/jade/lang/zh_tw.json"),
        _ => return None,
    };
    Some(bytes)
}
