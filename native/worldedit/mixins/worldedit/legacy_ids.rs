// Use the engine's already-loaded legacy mappings; no game-data table is bundled.
use std::sync::OnceLock;
pub fn legacy_state(id: usize) -> (&'static str, &'static str) {
    static TABLE: OnceLock<Vec<(String, String)>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let data = super::block_state_data::data();
        (0..4096).map(|id| {
            let state = data.state(data.instance(id));
            let properties = state.props.iter().map(|(k,v)| format!("{k}={v}")).collect::<Vec<_>>().join(";");
            (state.name.clone(), properties)
        }).collect()
    });
    table.get(id).map(|(name,props)| (name.as_str(),props.as_str())).unwrap_or(("minecraft:air", ""))
}
