use super::Hasher as H;
use tiny_keccak::{Hasher, Keccak};
pub struct KeccakHasher {}

impl KeccakHasher {
    pub fn new() -> Self {
        KeccakHasher {}
    }
}

impl H for KeccakHasher {
    type Hash = [u8; 32];

    fn hash(&self, data: &[u8]) -> Self::Hash {
        let mut keccak = Keccak::v256();
        keccak.update(data);
        let mut out: [u8; 32] = [0; 32];
        keccak.finalize(&mut out);
        out
    }

    // output size in bytes
    fn output_size(&self) -> usize {
        32
    }
}
