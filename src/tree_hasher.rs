pub trait Hasher {
    type Hash: Copy + PartialEq + Into<Vec<u8>> + TryFrom<Vec<u8>>;
    fn hash(&self, data: &[u8]) -> Self::Hash;
    fn output_size(&self) -> usize;
}

#[derive(Clone)]
pub struct TreeHasher<H: Hasher> {
    pub hasher: H,
    pub zero_hash: Vec<u8>,
}

impl<H: Hasher> TreeHasher<H> {
    const NODE_PREFIX: [u8; 1] = [1];
    const LEAF_PREFIX: [u8; 1] = [0];

    pub fn new(hasher: H) -> Self {
        let zero_hash = vec![0; hasher.output_size()];
        Self { hasher, zero_hash }
    }

    pub fn path(&self, key: &[u8]) -> Vec<u8> {
        self.hasher.hash(key).into()
    }

    pub fn digest(&self, data: &[u8]) -> Vec<u8> {
        self.hasher.hash(data).into()
    }

    pub fn digest_node(&self, left: &[u8], right: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let mut data = Self::NODE_PREFIX.to_vec();
        data.extend_from_slice(left);
        data.extend_from_slice(right);
        (self.hasher.hash(&data).into(), data)
    }

    pub fn digest_leaf(&self, path: &[u8], value: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let mut data = Self::LEAF_PREFIX.to_vec();
        data.extend_from_slice(path);
        data.extend_from_slice(value);
        (self.hasher.hash(&data).into(), data)
    }

    pub fn parse_leaf(&self, data: &[u8]) -> (Vec<u8>, Vec<u8>) {
        (
            data[Self::LEAF_PREFIX.len()..self.hasher.output_size() + Self::LEAF_PREFIX.len()]
                .to_vec(),
            data[self.hasher.output_size() + Self::LEAF_PREFIX.len()..].to_vec(),
        )
    }

    pub fn parse_node(&self, data: &[u8]) -> (Vec<u8>, Vec<u8>) {
        (
            data[Self::NODE_PREFIX.len()..self.hasher.output_size() + Self::LEAF_PREFIX.len()]
                .to_vec(),
            data[self.hasher.output_size() + Self::LEAF_PREFIX.len()..].to_vec(),
        )
    }

    pub fn is_leaf(&self, data: &[u8]) -> bool {
        data[..Self::LEAF_PREFIX.len()] == Self::LEAF_PREFIX
    }
}
