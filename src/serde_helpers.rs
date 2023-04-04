use ::once_cell::unsync::OnceCell;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub mod once_cell {
    use super::*;

    pub fn serialize<S>(cell: &OnceCell<impl Serialize>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        cell.get().serialize(serializer)
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<OnceCell<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        let val = Option::<T>::deserialize(deserializer)?;
        Ok(val.map_or_else(OnceCell::new, OnceCell::with_value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Test {
        #[serde(with = "once_cell")]
        once_cell: OnceCell<i32>,
    }

    #[test]
    fn serde_bincode() {
        let val = Test {
            once_cell: OnceCell::new(),
        };
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize::<Test>(&bytes).unwrap();
        assert_eq!(val, de_val);

        let val = Test {
            once_cell: OnceCell::with_value(100),
        };
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize::<Test>(&bytes).unwrap();
        assert_eq!(val, de_val);
    }
}
