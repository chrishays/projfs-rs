
// TODO: Interface for ordering
pub const VIRTUAL_FILES: &[(&str, bool, u8)] = &[
    ("", true, 0),
    ("ooo", false, 2),
    ("other", true, 0),
    ("other\\no", false, 10),
    ("zeros", true, 0),
    ("zeros\\0", false, 0),
    ("zeros\\1", false, 1),
    ("zeros\\2", false, 2),
    ("zeros\\3", false, 3),
    ("zeros\\4", false, 4),
    ("zeros\\5", false, 5),
    ("zeros\\6", false, 6),
    ("zeros\\7", false, 7),
    ("zeros\\8", false, 8),
    ("zeros\\9", false, 9),
];