thread_local! {
 pub static COORDINATES: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}
