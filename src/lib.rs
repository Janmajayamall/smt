pub mod keccak_hasher;
pub mod rocksdb_kv;
pub mod tree_hasher;
mod utils;

use self::utils::{common_prefix, get_msb_at, set_msb_at};
use std::{
    fmt::{self, Pointer},
    vec::Vec,
};
use tree_hasher::{Hasher, TreeHasher};

pub trait KvStore {
    fn get(&self, k: &[u8]) -> anyhow::Result<Vec<u8>>;
    fn insert(&self, k: &[u8], v: &[u8]) -> anyhow::Result<()>;
    fn delete(&self, k: &[u8]) -> anyhow::Result<()>;
}

pub struct SparseMerkleTree<H: Hasher, K: KvStore> {
    tree_hasher: TreeHasher<H>,
    nodes: K,
    values: K,
    root: Vec<u8>,
}

pub struct SparseMerkleProof {
    /// Sidenodes for key down from root
    /// to bottom of the tree
    sidenodes: Vec<Vec<u8>>,
    /// Used for proving the key value is
    /// non existant
    non_membership_leaf_node: Vec<u8>,
}

pub struct SparseMerkleCompactProof {
    /// Sidenodes for key down from root
    /// to bottom of the tree after excluding
    /// all placeholders
    compact_sidenodes: Vec<Vec<u8>>,
    /// Used for proving the key value is
    /// non existant
    non_membership_leaf_node: Vec<u8>,
    /// Bitmask indicating at which index
    /// (i.e. index corresponding to 1 bit) to
    /// use placeholders for proof
    bitmask: Vec<u8>,
}

impl<H: Hasher, K: KvStore> SparseMerkleTree<H, K> {
    const DEFAULT_VALUE: Vec<u8> = vec![];

    pub fn new(tree_hasher: TreeHasher<H>, nodes_store: K, values_store: K) -> Self {
        Self {
            root: tree_hasher.zero_hash.clone(),
            tree_hasher,
            nodes: nodes_store,
            values: values_store,
        }
    }

    pub fn get(&self, key: &[u8]) -> anyhow::Result<Vec<u8>> {
        if self.root == self.placeholder() {
            Ok(vec![0])
        } else {
            let path = self.tree_hasher.path(key);
            self.values.get(&path)
        }
    }

    pub fn sidenodes(
        &self,
        root: &[u8],
        path: &[u8],
    ) -> anyhow::Result<(Vec<Vec<u8>>, Vec<Vec<u8>>, Vec<u8>)> {
        // keys of the nodes
        let mut sidenodes = Vec::<Vec<u8>>::new();
        // root is by default part of the path
        let mut pathnodes = vec![root.to_vec()];

        if root == self.placeholder() {
            return Ok((sidenodes, pathnodes, Self::DEFAULT_VALUE));
        }

        // Node corresponding to root hash should exist
        let mut node = self.nodes.get(root)?;
        if self.tree_hasher.is_leaf(&node) {
            // if root is leaf, then it does not have
            // sidenodes
            return Ok((sidenodes, pathnodes, node));
        }

        let mut k_pathnode: Vec<u8>;
        let mut k_sidenode: Vec<u8>;

        for i in 0..self.depth() {
            let (left, right) = self.tree_hasher.parse_node(&node);
            if get_msb_at(path, i) == 0 {
                // left traversal
                k_pathnode = left;
                k_sidenode = right;
            } else {
                // right traversal
                k_pathnode = right;
                k_sidenode = left;
            }

            sidenodes.push(k_sidenode);
            pathnodes.push(k_pathnode.clone());

            if k_pathnode == self.placeholder() {
                node = Self::DEFAULT_VALUE;
                break;
            }

            // Get pathnode using k_pathnode
            node = self.nodes.get(&k_pathnode)?;
            if self.tree_hasher.is_leaf(&node) {
                break;
            }
        }

        sidenodes.reverse();
        pathnodes.reverse();
        Ok((sidenodes, pathnodes, node))
    }

    fn _update(
        &self,
        path: &[u8],
        value: &[u8],
        sidenodes: &[Vec<u8>],
        pathnodes: &[Vec<u8>],
        // old_data is non-default only when pathnode[0] is a leaf.
        old_data: &[u8],
    ) -> anyhow::Result<Vec<u8>> {
        // println!("sidenodes {:#x?}", sidenodes);
        // println!("pathnodes {:x?}", pathnodes);
        // println!("olddata {:x?}", old_data);

        // Create leaf node for new value
        let val_hash = self.tree_hasher.digest(value);
        let (mut curr_hash, mut curr_data) = self.tree_hasher.digest_leaf(path, &val_hash);
        // println!(
        //     "path {:x?} node hash {:x?} node data {:x?}",
        //     path, curr_hash, curr_data
        // );
        self.nodes.insert(&curr_hash, &curr_data)?;

        // If pathnode at index 0 is a placeholder
        // then we can simply replace it with the new
        // node as the leaf.
        // If pathnode is a leaf, then it either
        // has same path or different path.
        // In case of same path we must delete the
        // node and replace it with new node and update
        // the nodes along the path.
        // In case of different paths we must first find the
        // the length of the common path, create two different
        // subtrees (with rest empty nodes) for each and extend
        // current tree with siblings as placeholders till parent node
        // of the subtrees.
        let common_prefix_len;
        let mut pathnode_value_hash = Self::DEFAULT_VALUE;
        if pathnodes[0] == self.placeholder() {
            common_prefix_len = self.depth();
        } else {
            // Node at the end of path there is either
            // another leaf OR same leaf. If it's another leaf
            // then we must extend the subtree until
            // the new node and exisiting leaf node aren't
            // in different subtrees with rest of the nodes
            // as empty nodes.
            //
            // extension_length = common_prefix_len - sidenodes.len()
            let pathnode_path;
            (pathnode_path, pathnode_value_hash) = self.tree_hasher.parse_leaf(old_data);
            common_prefix_len = common_prefix(&pathnode_path, path);
            // println!("common_prefix_len = {}", common_prefix_len);
        }

        if common_prefix_len != self.depth() {
            // create 2 new subtrees and calc their (parent) internal node
            // println!("bit value {}", get_msb_at(path, common_prefix_len));
            if get_msb_at(path, common_prefix_len) == 0 {
                // left
                (curr_hash, curr_data) = self.tree_hasher.digest_node(&curr_hash, &pathnodes[0]);
            } else {
                // right
                (curr_hash, curr_data) = self.tree_hasher.digest_node(&pathnodes[0], &curr_hash);
            }
            // println!("node hash {:x?} node data {:x?}", curr_hash, curr_data);
            self.nodes.insert(&curr_hash, &curr_data)?;
        } else if pathnode_value_hash != Self::DEFAULT_VALUE {
            // If val hash of leaf at path end is
            // same as val hash we are trying to add,
            // then return exisitng root since there's
            // no actual update.
            if pathnode_value_hash == val_hash {
                return Ok(self.root.clone());
            }

            // Otherwise delete existing value
            self.nodes.delete(&pathnodes[0])?;
            self.values.delete(path)?;
        }

        // Delete pathnodes since they will be
        // updated right after.
        for (i, node) in pathnodes.iter().enumerate() {
            if i != 0 {
                self.nodes.delete(node)?;
            }
        }

        let leaf_offset = self.depth() - sidenodes.len();
        let mut sidenode;
        for i in 0..self.depth() {
            if i < leaf_offset {
                if common_prefix_len != self.depth() && common_prefix_len > self.depth() - i - 1 {
                    // Since common_prefix is greater than depth
                    // extend the tree using placeholder as sidenodes.

                    sidenode = self.placeholder();
                } else {
                    continue;
                }
            } else {
                sidenode = sidenodes[i - leaf_offset].clone();
            }

            if get_msb_at(path, self.depth() - i - 1) == 0 {
                (curr_hash, curr_data) = self.tree_hasher.digest_node(&curr_hash, &sidenode);
            } else {
                (curr_hash, curr_data) = self.tree_hasher.digest_node(&sidenode, &curr_hash);
            }

            self.nodes.insert(&curr_hash, &curr_data)?;
        }

        // set value
        self.values.insert(path, value)?;
        Ok(curr_hash)
    }

    fn _delete(
        &self,
        path: &[u8],
        sidenodes: &[Vec<u8>],
        pathnodes: &[Vec<u8>],
    ) -> anyhow::Result<Vec<u8>> {
        // println!("path {:x?}", path);
        // println!("sidenodes {:x?}", sidenodes);
        // println!("pathnodes {:x?}", pathnodes);

        // If the node at `pathnodes[0]` is placeholder
        // then return since no value exists at `path`
        if pathnodes[0] == self.placeholder() {
            return Ok(self.root.clone());
        }

        // delete all pathnodes
        for i in pathnodes {
            self.nodes.delete(i)?;
        }

        // On deleting the leaf node we turn the
        // node into a placeholder. Therefore, we must
        // contract tree (i.e. bubble up) until a non-placeholder
        // sibling.
        // Note that if first non-placholder sibling is leaf,
        // it must be bubbled up again (since we collapsee a subtree
        // with one node into a single node). Otherwise the node
        // must be left in-place.
        let mut curr_hash = self.placeholder();
        let mut curr_data = Vec::<u8>::new();
        let mut flag: bool = false;
        for i in 0..sidenodes.len() {
            if !flag {
                if sidenodes[i] != self.placeholder() {
                    if curr_hash == self.placeholder()
                        && self.tree_hasher.is_leaf(&self.nodes.get(&sidenodes[i])?)
                    {
                        // Sidenode is a leaf, sp bubble up till next non-placholder
                        curr_hash = sidenodes[i].clone();
                        continue;
                    } else {
                        // Sidenode is internal node, so leave it in-place
                        // and calculate nodes above with curr_hash as placeholder
                        flag = true;
                    }
                } else {
                    continue;
                }
            }

            if get_msb_at(path, sidenodes.len() - i - 1) == 0 {
                (curr_hash, curr_data) = self.tree_hasher.digest_node(&curr_hash, &sidenodes[i]);
            } else {
                (curr_hash, curr_data) = self.tree_hasher.digest_node(&sidenodes[i], &curr_hash);
            }
            self.nodes.insert(&curr_hash, &curr_data)?;
        }

        // delete value at path
        self.values.delete(path)?;

        Ok(curr_hash)
    }

    pub fn update(&mut self, key: &[u8], value: &[u8]) -> anyhow::Result<Vec<u8>> {
        self.update_for_root(key, value)
    }

    pub fn delete(&mut self, key: &[u8]) -> anyhow::Result<Vec<u8>> {
        self.update_for_root(key, &Self::DEFAULT_VALUE)
    }

    fn update_for_root(&mut self, key: &[u8], value: &[u8]) -> anyhow::Result<Vec<u8>> {
        let path = self.tree_hasher.path(key);
        let (sidenodes, pathnodes, old_data) = self.sidenodes(&self.root, &path)?;

        if value == Self::DEFAULT_VALUE {
            self.root = self._delete(&path, &sidenodes, &pathnodes)?;
        } else {
            self.root = self._update(&path, value, &sidenodes, &pathnodes, &old_data)?;
        }
        Ok(self.root.clone())
    }

    fn depth(&self) -> usize {
        self.tree_hasher.hasher.output_size() * 8
    }

    fn placeholder(&self) -> Vec<u8> {
        self.tree_hasher.zero_hash.clone()
    }

    /// Generates `SparseMerkleProof` for a `key` in the
    /// `root`.
    pub fn generate_proof(&self, key: &[u8], root: &[u8]) -> anyhow::Result<SparseMerkleProof> {
        let path = self.tree_hasher.path(key);

        let (sidenodes, pathnodes, leaf_data) = self.sidenodes(root, &path)?;

        let mut non_membership_leaf_node = Vec::<u8>::new();
        // If `pathnodes[0]` is a placeholder, it means
        // value corresponding to key is nil in the tree
        if pathnodes[0] != self.placeholder() {
            let (leaf_path, _) = self.tree_hasher.parse_leaf(&leaf_data);
            // If `path` does not match with `leaf_path`, then `leaf_data`
            // corresponds to some key other than give `key`. This proves
            // that value corresponding to `key` is non existent.
            if leaf_path != path {
                non_membership_leaf_node = pathnodes[0].clone();
            }
        }

        Ok(SparseMerkleProof {
            sidenodes,
            non_membership_leaf_node,
        })
    }

    pub fn generate_compact_proof(
        &self,
        key: &[u8],
        root: &[u8],
    ) -> anyhow::Result<SparseMerkleCompactProof> {
        let proof = self.generate_proof(key, root)?;

        let mut compact_sidenodes = Vec::<Vec<u8>>::new();
        let mut bitmask = Vec::<u8>::new();
        for (index, s) in proof.sidenodes.iter().enumerate() {
            if *s != self.placeholder() {
                compact_sidenodes.push(s.clone());
            } else {
                set_msb_at(&mut bitmask, index);
            }
        }

        Ok(SparseMerkleCompactProof {
            compact_sidenodes,
            bitmask,
            non_membership_leaf_node: proof.non_membership_leaf_node,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        keccak_hasher::KeccakHasher, rocksdb_kv::RocksDbStore, SparseMerkleTree, TreeHasher,
    };

    #[test]
    fn new() {
        let hasher = KeccakHasher::new();
        let tree_hasher = TreeHasher::new(hasher);
        let nodes = RocksDbStore::new("./db/nodes");
        let values = RocksDbStore::new("./db/values");
        let mut smt = SparseMerkleTree::new(tree_hasher, nodes, values);

        let k1 = b"k1";
        let v1 = b"v1";
        let k2 = b"k2";
        let v2 = b"v2";

        println!("Update K1");
        let res = smt.update(k1, v1).unwrap();
        println!("root {:x?} ", res);
        let res = smt.get(k1).unwrap();
        assert!(res == v1);

        println!("Update K2");
        let res = smt.update(k2, v2).unwrap();
        println!("root {:x?} ", res);
        let res = smt.get(k2).unwrap();
        assert!(res == v2);

        println!("Delete k2...");
        let res = smt.delete(k2).unwrap();
        println!("root {:x?} ", res);
        // println!("root 2  {:x?} {:x?}", root1, res.unwrap());
        // assert!(root1 == res.unwrap());

        // println!("root {:x?}", smt.root);
    }
}
