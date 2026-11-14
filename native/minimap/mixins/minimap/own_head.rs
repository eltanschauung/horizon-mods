fn own_head(result:bool)->bool { result && !minimap_coordinates::OWN_HEAD.with(|v|v.get()) }
