//! Built-in character avatars, embedded in the binary.
//!
//! The art lives in `assets/avatars/<name>.png` (placeholders from
//! `cargo run --bin genavatars`). To swap in real artwork, overwrite a PNG with
//! any square image and rebuild. Profiles store the character *name*
//! (`ProfileManifest::character`), so every profile picks up the new art.
//! Adding a character = drop in a PNG and add a line here. Never rename or
//! remove an existing name: profiles on disk refer to it (an unknown name falls
//! back to the drawn default face).

pub const CHARACTERS: &[(&str, &[u8])] = &[
    ("fox", include_bytes!("../../../assets/avatars/fox.png")),
    ("cat", include_bytes!("../../../assets/avatars/cat.png")),
    ("bear", include_bytes!("../../../assets/avatars/bear.png")),
    ("frog", include_bytes!("../../../assets/avatars/frog.png")),
    ("owl", include_bytes!("../../../assets/avatars/owl.png")),
    ("bunny", include_bytes!("../../../assets/avatars/bunny.png")),
    ("robot", include_bytes!("../../../assets/avatars/robot.png")),
    ("dino", include_bytes!("../../../assets/avatars/dino.png")),
];

/// The embedded PNG for character `name`, if it exists.
pub fn png(name: &str) -> Option<&'static [u8]> {
    CHARACTERS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, bytes)| *bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_character_decodes_as_a_square_image() {
        for (name, bytes) in CHARACTERS {
            let img = image::load_from_memory(bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(img.width(), img.height(), "{name} must be square");
            assert!(img.width() >= 128, "{name} is too small");
        }
    }

    #[test]
    fn names_are_unique() {
        let mut names: Vec<_> = CHARACTERS.iter().map(|(n, _)| *n).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), CHARACTERS.len());
    }
}
