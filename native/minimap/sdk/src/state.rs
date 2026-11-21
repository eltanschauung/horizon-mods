thread_local! {
 pub static COORDINATES: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}
thread_local! {
 pub static OUTLINE: std::cell::Cell<(usize, bool)> = const { std::cell::Cell::new((0, false)) };
}

#[derive(Clone)]
pub struct Marker {
    pub x: f32,
    pub y: f32,
    pub face: Option<[f32; 4]>,
    pub hat: Option<[f32; 4]>,
    pub color: u32,
    pub label: String,
    pub size: f32,
}
thread_local! {
 pub static MARKERS: std::cell::RefCell<Vec<Marker>> = const { std::cell::RefCell::new(Vec::new()) };
 pub static OWN_HEAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
thread_local! {
    pub static COMPASS_NORTH: std::cell::Cell<Option<(f32, f32)>> = const { std::cell::Cell::new(None) };
}
