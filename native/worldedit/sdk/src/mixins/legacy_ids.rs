//! Legacy IDs from the installed prepared resources.
use std::path::Path;
use std::sync::OnceLock;

type Table = Vec<Option<(String, String)>>;
static TABLE: OnceLock<Table> = OnceLock::new();

pub(super) fn table(root: &Path) -> Result<&'static Table, String> {
    if let Some(table) = TABLE.get() {
        return Ok(table);
    }
    #[derive(serde::Deserialize)]
    struct Tables {
        block_state_data: Vec<String>,
    }
    let file = std::fs::File::open(root.join("datafix-tables.json"))
        .map_err(|e| format!("legacy block mappings: {e}"))?;
    let tables: Tables = serde_json::from_reader(std::io::BufReader::new(file))
        .map_err(|e| format!("legacy block mappings: {e}"))?;
    let mut exact = vec![None; 4096];
    let mut first = vec![None; 256];
    for line in &tables.block_state_data {
        let mut fields = line.split('\t');
        let Some(id) = fields.next().and_then(|field| field.parse::<usize>().ok()).filter(|&id| id < 4096)
        else { continue };
        let Some(name) = fields.next() else { continue };
        let state = (name.to_owned(), fields.next().unwrap_or("").to_owned());
        first[id >> 4].get_or_insert_with(|| state.clone());
        exact[id] = Some(state);
    }
    if exact[0].is_none() {
        return Err("the prepared legacy block table is incomplete".into());
    }
    for (id, state) in exact.iter_mut().enumerate() {
        if state.is_none() {
            *state = first[id >> 4].clone();
        }
    }
    Ok(TABLE.get_or_init(|| exact))
}
