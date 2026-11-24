fn suppress_own_arrow(visible: bool) -> bool {
    visible && !minimap::state::OWN_HEAD.get()
}
