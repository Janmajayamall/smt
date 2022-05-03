use super::KvStore;
use rocksdb::DB;

pub struct RocksDbStore {
    db: DB,
}

impl RocksDbStore {
    pub fn new(path: &str) -> Self {
        let db = DB::open_default(path).expect("DB Path must resolve");
        Self { db }
    }
}

impl KvStore for RocksDbStore {
    fn get(&self, k: &[u8]) -> anyhow::Result<Vec<u8>> {
        self.db.get(k).map_err(|e| e.into()).and_then(|r| {
            if let Some(r) = r {
                Ok(r)
            } else {
                Err(anyhow::anyhow!("Key record {:?} does not exist!", k))
            }
        })
    }

    fn insert(&self, k: &[u8], v: &[u8]) -> anyhow::Result<()> {
        // println!("inseting k {:?} v {:?}", k, v);
        self.db.put(k, v).map_err(|e| e.into())
    }

    fn delete(&self, k: &[u8]) -> anyhow::Result<()> {
        self.db.delete(k).map_err(|e| e.into())
    }
}
