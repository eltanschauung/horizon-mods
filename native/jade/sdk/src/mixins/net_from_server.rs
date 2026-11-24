fn jade_replies() {
    for data in ::jade::take_replies() {
        crate::game::jade::from_server(::jade::DATA, &data);
    }
}
