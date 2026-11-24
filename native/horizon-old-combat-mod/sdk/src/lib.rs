// SPDX-License-Identifier: MIT
use mod_api::{
    BoolGameRuleKey, Hooks, HostSide, PlayerTickPostEvent, Registrar, RegistrationContext,
};
use std::sync::OnceLock;

const ATTACK_SPEED: &str = "minecraft:attack_speed";
const CLASSIC_ATTACK_SPEED: f64 = 1024.0;

#[derive(Default)]
struct OldCombat {
    enabled: OnceLock<BoolGameRuleKey>,
}

#[mod_api::horizon_mod]
impl Hooks for OldCombat {
    fn register(&self, registrar: &mut dyn Registrar, _: &RegistrationContext) {
        if let Ok(key) = registrar.game_rule_bool("old_combat", "minecraft:player", true) {
            let _ = self.enabled.set(key);
        }
    }

    fn player_tick_post(&self, host: HostSide, event: &mut PlayerTickPostEvent) {
        let (HostSide::Server(host), Some(&enabled)) = (host, self.enabled.get()) else {
            return;
        };
        let player = event.player;
        let Some(level) = player.level(host) else {
            return;
        };
        let Some(enabled) = level.game_rule_bool(host, enabled) else {
            return;
        };
        let Some(living) = host.living(player.entity()) else {
            return;
        };
        let Some(current) = living.attribute_base(host, ATTACK_SPEED) else {
            return;
        };
        if let Some(speed) = changed_base(enabled, current) {
            living.set_attribute_base(host, ATTACK_SPEED, speed);
        }
    }
}

fn changed_base(enabled: bool, current: f64) -> Option<f64> {
    if enabled && current != CLASSIC_ATTACK_SPEED {
        Some(CLASSIC_ATTACK_SPEED)
    } else if !enabled && current == CLASSIC_ATTACK_SPEED {
        Some(4.0)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_updates_only_when_the_rule_requires_it() {
        assert_eq!(changed_base(true, 4.0), Some(CLASSIC_ATTACK_SPEED));
        assert_eq!(changed_base(true, CLASSIC_ATTACK_SPEED), None);
        assert_eq!(changed_base(false, CLASSIC_ATTACK_SPEED), Some(4.0));
        assert_eq!(changed_base(false, 7.0), None);
    }
}
