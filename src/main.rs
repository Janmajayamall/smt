use node::Node;
use serde::{Deserialize, Serialize};
use std::{io::Error, vec::Vec};
use utils::common_prefix;

mod node;
mod utils;
trait KvStore {
    fn get(&self, k: &[u8]) -> Vec<u8>;
    fn insert(&self, k: &[u8], v: &[u8]);
    fn delete(&self, k: &[u8]);
}

trait Hasher {
    type Hash: Copy + PartialEq + Into<Vec<u8>> + TryFrom<Vec<u8>>;
    fn hash(&self, data: &[u8]) -> Self::Hash;
    fn output_size(&self) -> usize;
}

#[derive(Clone)]
struct TreeHasher<H: Hasher> {
    hasher: H,
    zero_hash: Vec<u8>,
}

impl<H: Hasher> TreeHasher<H> {
    const NODE_PREFIX: [u8; 1] = [1];
    const LEAF_PREFIX: [u8; 1] = [0];

    fn path(&self, key: &[u8]) -> Vec<u8> {
        self.hasher.hash(key).into()
    }

    pub fn digest(&self, data: &[u8]) -> Vec<u8> {
        self.hasher.hash(&data).into()
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
        data[..Self::NODE_PREFIX.len()] == Self::NODE_PREFIX
    }
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

    /**
     * 1. Check that root is not default -
     *      If it is default, then return default value (i.e. vec![0])
     * 2. Convert key to path and then the respective value
     */
    pub fn get(&self, key: &[u8]) -> Vec<u8> {
        if self.root == self.tree_hasher.zero_hash {
            vec![0]
        } else {
            let path = self.tree_hasher.hasher.hash(key);
            self.values.get(&path.into())
        }
    }

    pub fn sidenodes(
        &self,
        root: &[u8],
        path: &[u8],
    ) -> anyhow::Result<(Vec<Vec<u8>>, Vec<Vec<u8>>)> {
        // keys of the nodes
        let mut sidenodes = Vec::<Vec<u8>>::new();
        let mut pathnodes = Vec::<Vec<u8>>::new();
        // root is by default part of the path
        pathnodes.push(root.to_vec());

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
            common_prefix_len = common_prefix(&pathnodes[0], path);
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

        // TODO delete path nodes at indexes 1..

        for node in pathnodes {
            self.nodes.delete(node);
            // TODO also delete values
        }

        // for p in 0...self.depth() {

        // }

        // set value
        self.values.insert(path, value);
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
