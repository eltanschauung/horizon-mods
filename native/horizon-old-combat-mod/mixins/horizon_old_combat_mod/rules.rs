// SPDX-License-Identifier: MIT
fn register_rules(rules: &mut Vec<Rule>) {
    rules.push(Rule { name: "old_combat", category: "player", default: Value::Bool(true), min: 0, max: 0 });
}
