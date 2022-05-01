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
    ) -> anyhow::Result<(Vec<Vec<u8>>, Vec<Vec<u8>>)> {
        // keys of the nodes
        let mut sidenodes = Vec::<Vec<u8>>::new();
        // root is by default part of the path
        let mut pathnodes = vec![root.to_vec()];

        if root == self.placeholder() {
            return Ok((sidenodes, pathnodes));
        }

        // Node corresponding to root hash should exist
        let mut node = self.nodes.get(root);
        if self.tree_hasher.is_leaf(&node) {
            // if root is leaf, then it does not have
            // sidenodes
            return Ok((sidenodes, pathnodes));
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
        Ok((sidenodes, pathnodes))
    }

    pub fn update(&self, path: &[u8], value: &[u8], sidenodes: &[Vec<u8>], pathnodes: &[Vec<u8>]) {
        // Create leaf node for new value
        let val_hash = self.tree_hasher.digest(value);
        let (mut curr_data, mut curr_val) = self.tree_hasher.digest_leaf(path, &val_hash);
        self.nodes.insert(&curr_data, &curr_val);

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
        let mut common_prefix_len;
        if pathnodes[0] == self.placeholder() {
            common_prefix_len = self.depth();
        } else {
            // Node at the bottom of path is another leaf.
            // Therefore, we must extend the subtree until
            // the new node and exisiting leaf node aren't
            // in different subtrees with rest of the nodes
            // as empty nodes (i.e. extend until they have
            //
            // extension_length = common_prefix_len - sidenodes.len()
            common_prefix_len = common_prefix(&pathnodes[0], path) - sidenodes.len();
        }

        if common_prefix_len != self.depth() {
            // create 2 new subtrees and get their parent node
            if path[common_prefix_len] == 0 {
                // left
                (curr_data, curr_val) = self.tree_hasher.digest_node(&curr_data, &pathnodes[0]);
            } else {
                // right
                (curr_data, curr_val) = self.tree_hasher.digest_node(&pathnodes[0], &curr_data);
            }
            self.nodes.insert(&curr_data, &curr_val);
        } else {
            // if value exists then delete it
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
                    // extend the tree with placholder as the sidenode
                    sidenode = self.placeholder();
                } else {
                    continue;
                }
            } else {
                sidenode = sidenodes[i - leaf_offset].clone();
            }

            if path[self.depth() - i - 1] == 0 {
                (curr_data, curr_val) = self.tree_hasher.digest_node(&curr_data, &sidenode);
            } else {
                (curr_data, curr_val) = self.tree_hasher.digest_node(&sidenode, &curr_data);
            }

            self.nodes.insert(&curr_data, &curr_val);
        }

        // set value
        self.values.insert(path, value);
    }

    pub fn delete(&self, path: &[u8], sidenodes: &[Vec<u8>], pathnodes: &[Vec<u8>]) {
        // If the node at path isn't leaf
        // then we must return
        if !self.tree_hasher.is_leaf(&pathnodes[0]) {
            return;
        }

        // delete all pathnodes
        for i in pathnodes {
            self.nodes.delete(i);
        }

        // On deleting the leaf node we turn the
        // node into a placeholder. Therefore, we must
        // contract tree (i.e. bubble up) until a non-placeholder
        // sibling.
        let mut curr_data = Vec::<u8>::new();
        let mut curr_val = Vec::<u8>::new();
        let mut flag: bool = false;
        for i in 0..sidenodes.len() {
            if !flag {
                if sidenodes[i] != self.placeholder() {
                    // found a non-placeholder sibling
                    curr_data = sidenodes[i].to_vec();
                    flag = true;
                }
                continue;
            }

            if path[sidenodes.len() - i - 1] == 0 {
                (curr_data, curr_val) = self.tree_hasher.digest_node(&curr_data, &sidenodes[i]);
                self.nodes.insert(&curr_data, &curr_val);
            }
        }

        // delete value at path
        self.values.delete(path);
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
