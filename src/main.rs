use std::{io::Error, vec::Vec};
use tree_hasher::{Hasher, TreeHasher};
use utils::common_prefix;

mod node;
mod tree_hasher;
mod utils;
trait KvStore {
    fn get(&self, k: &[u8]) -> Vec<u8>;
    fn insert(&self, k: &[u8], v: &[u8]);
    fn delete(&self, k: &[u8]);
}

struct SparseMerkleTree<H: Hasher, K: KvStore + Default> {
    tree_hasher: TreeHasher<H>,
    nodes: K,
    values: K,
    root: Vec<u8>,
}

impl<H: Hasher, K: KvStore + Default> SparseMerkleTree<H, K> {
    const DEFAULT_VALUE: Vec<u8> = vec![];

    pub fn new(tree_hasher: TreeHasher<H>) -> Self {
        Self {
            root: tree_hasher.zero_hash.clone(),
            tree_hasher,
            nodes: Default::default(),
            values: Default::default(),
        }
    }

    pub fn get(&self, key: &[u8]) -> Vec<u8> {
        if self.root == self.placeholder() {
            vec![0]
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
        let mut node = self.nodes.get(root);
        if self.tree_hasher.is_leaf(&node) {
            // if root is leaf, then it does not have
            // sidenodes
            return Ok((sidenodes, pathnodes, Self::DEFAULT_VALUE));
        }

        let mut k_pathnode: Vec<u8>;
        let mut k_sidenode: Vec<u8>;

        for p in path.iter().take(self.depth()) {
            let (left, right) = self.tree_hasher.parse_node(&node);
            if *p == 0 {
                // 0 is left traversal
                k_pathnode = left;
                k_sidenode = right;
            } else {
                // 1 is right traversal
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
            node = self.nodes.get(&k_pathnode);
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
    ) -> Vec<u8> {
        // Create leaf node for new value
        let val_hash = self.tree_hasher.digest(value);
        let (mut curr_hash, mut curr_data) = self.tree_hasher.digest_leaf(path, &val_hash);
        self.nodes.insert(&curr_hash, &curr_data);

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
        }

        if common_prefix_len != self.depth() {
            // create 2 new subtrees and calc their (parent) internal node
            if path[common_prefix_len] == 0 {
                // left
                (curr_hash, curr_data) = self.tree_hasher.digest_node(&curr_hash, &pathnodes[0]);
            } else {
                // right
                (curr_hash, curr_data) = self.tree_hasher.digest_node(&pathnodes[0], &curr_hash);
            }
            self.nodes.insert(&curr_hash, &curr_data);
        } else if pathnode_value_hash != Self::DEFAULT_VALUE {
            // If val hash of leaf at path end is
            // same as val hash we are trying to add,
            // then return exisitng root since there's
            // no actual update.
            if pathnode_value_hash == val_hash {
                return self.root.clone();
            }

            // Otherwise delete existing value
            self.nodes.delete(&pathnodes[0]);
            self.values.delete(path);
        }

        // Delete pathnodes since they will be
        // updated right after.
        for (i, node) in pathnodes.iter().enumerate() {
            if i != 0 {
                self.nodes.delete(node);
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

            if path[self.depth() - i - 1] == 0 {
                (curr_hash, curr_data) = self.tree_hasher.digest_node(&curr_hash, &sidenode);
            } else {
                (curr_hash, curr_data) = self.tree_hasher.digest_node(&sidenode, &curr_hash);
            }

            self.nodes.insert(&curr_hash, &curr_data);
        }

        // set value
        self.values.insert(path, value);

        curr_hash
    }

    fn _delete(&self, path: &[u8], sidenodes: &[Vec<u8>], pathnodes: &[Vec<u8>]) -> Vec<u8> {
        // If the node at path isn't leaf
        // then we must return
        if !self.tree_hasher.is_leaf(&pathnodes[0]) {
            return self.root.clone();
        }

        // delete all pathnodes
        for i in pathnodes {
            self.nodes.delete(i);
        }

        // On deleting the leaf node we turn the
        // node into a placeholder. Therefore, we must
        // contract tree (i.e. bubble up) until a non-placeholder
        // sibling.
        let mut curr_hash = Vec::<u8>::new();
        let mut curr_data = Vec::<u8>::new();
        let mut flag: bool = false;
        for i in 0..sidenodes.len() {
            if !flag {
                if sidenodes[i] != self.placeholder() {
                    // We found the first non-placeholder
                    // sibling
                    flag = true;

                    curr_hash = sidenodes[i].clone();
                }
                continue;
            }

            if path[sidenodes.len() - i - 1] == 0 {
                (curr_hash, curr_data) = self.tree_hasher.digest_node(&curr_hash, &sidenodes[i]);
            } else {
                (curr_hash, curr_data) = self.tree_hasher.digest_node(&sidenodes[i], &curr_hash);
            }
            self.nodes.insert(&curr_hash, &curr_data);
        }

        // delete value at path
        self.values.delete(path);

        curr_hash
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
            self.root = self._delete(&path, &sidenodes, &pathnodes);
        } else {
            self.root = self._update(&path, value, &sidenodes, &pathnodes, &old_data);
        }
        Ok(self.root.clone())
    }

    fn depth(&self) -> usize {
        self.tree_hasher.hasher.output_size()
    }

    fn placeholder(&self) -> Vec<u8> {
        self.tree_hasher.zero_hash.clone()
    }
}

fn main() {
    println!("Hello, world!");
}
