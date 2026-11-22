fn strip_sculk(value: String) -> String {
    crate::no_sculk::edit(&value).unwrap_or(value)
}
