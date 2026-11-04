// SPDX-License-Identifier: MIT
fn no_copper_generation(
    overrides: &WorldgenOverrides,
    bundle: &str,
    mut value: BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    if overrides.mod_enabled(no_copper::ID) {
        no_copper::rewrite(bundle, &mut value);
    }
    value
}
