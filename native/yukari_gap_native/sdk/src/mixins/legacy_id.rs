fn upgrade_gap_type(id: Option<String>) -> Option<String> {
    id.map(|id| {
        if id == "yukari_gap:yukari_gap" {
            mod_yukari_gap_native::GAP_ENTITY.into()
        } else {
            id
        }
    })
}
