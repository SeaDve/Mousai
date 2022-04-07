#[derive(Clone, Hash, PartialEq, Eq)]
pub struct SongId {
    id: Box<str>,
}

impl std::fmt::Debug for SongId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SongId").field(&self.id).finish()
    }
}

impl std::fmt::Display for SongId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.id, f)
    }
}

impl SongId {
    pub(super) fn new(info_link: &str) -> Self {
        Self {
            id: Box::from(info_link),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn hash_map() {
        let mut hash_map = HashMap::new();

        let id_0 = SongId::new("Id0");
        hash_map.insert(&id_0, 0);

        let id_1 = SongId::new("Id1");
        hash_map.insert(&id_1, 1);

        let id_2 = SongId::new("Id2");
        hash_map.insert(&id_2, 2);

        assert_eq!(hash_map.get(&id_0), Some(&0));
        assert_eq!(hash_map.get(&id_1), Some(&1));
        assert_eq!(hash_map.get(&SongId::new("Id2")), Some(&2));
    }
}
