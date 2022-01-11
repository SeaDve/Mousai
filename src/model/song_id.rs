use std::{ffi::OsStr, path::Path};

#[derive(Hash, PartialEq, Eq)]
pub struct SongId {
    id: Box<OsStr>,
}

impl std::fmt::Debug for SongId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.id, f)
    }
}

impl SongId {
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        todo!();
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn hash_map() {
        let mut hash_map = HashMap::new();

        let id_0 = SongId::from_path("Path0");
        hash_map.insert(&id_0, 0);

        let id_1 = SongId::from_path("Path1");
        hash_map.insert(&id_1, 1);

        let id_2 = SongId::from_path("Path2");
        hash_map.insert(&id_2, 2);

        assert_eq!(hash_map.get(&id_0), Some(&0));
        assert_eq!(hash_map.get(&id_1), Some(&1));
        assert_eq!(hash_map.get(&SongId::from_path("Path2")), Some(&2));
    }
}
