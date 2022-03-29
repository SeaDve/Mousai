#[derive(Hash, PartialEq, Eq)]
pub struct SongId {
    id: Box<str>,
}

impl std::fmt::Debug for SongId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.id, f)
    }
}

impl SongId {
    pub(super) fn new(info_link: &str) -> Self {
        Self {
            id: Box::from(info_link),
        }
    }

    pub fn try_to_string(&self) -> anyhow::Result<String> {
        self.id
            .split('/')
            .last()
            .map(|last| last.to_string())
            .ok_or_else(|| anyhow::anyhow!("Invalid Song Id"))
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
