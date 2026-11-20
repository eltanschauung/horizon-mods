fn remove_copper(bundle: &str, mut value: BTreeMap<String, String>) -> BTreeMap<String, String> {
    mod_no_copper::rewrite(bundle, &mut value);
    value
}
