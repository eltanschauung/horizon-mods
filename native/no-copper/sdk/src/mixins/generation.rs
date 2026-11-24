fn remove_copper(bundle: &str, base: &mut BTreeMap<String, String>) {
    mod_no_copper::rewrite(bundle, base);
}
