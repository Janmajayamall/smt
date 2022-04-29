use serde::{Deserialize, Serialize};
use std::{io::Error, vec::Vec};

trait KvStore {
    fn get(&self, k: &[u8]) -> Vec<u8>;
    fn insert(&self, k: &[u8], v: &[u8]);
}

trait Hasher {
    type Hash: Copy + PartialEq + Into<Vec<u8>> + TryFrom<Vec<u8>>;
    fn hash(&self, data: &[u8]) -> Self::Hash;
    fn output_size(&self) -> usize;
}

#[derive(Deserialize, Serialize, PartialEq, Clone)]
enum Node {
    BranchNode { right: Vec<u8>, left: Vec<u8> },
    LeafNode { path: Vec<u8>, value: Vec<u8> },
    Empty,
}

impl TryFrom<&[u8]> for Node {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> anyhow::Result<Node> {
        // empty nodes are represnted using 0 bytes
        if value == vec![] {
            return Ok(Node::Empty);
        }
        bincode::deserialize(value).map_err(|e| e.into())
    }
}

impl Node {
    pub fn is_leaf(&self) -> bool {
        match self {
            Node::LeafNode { path: _, value: _ } => true,
            _ => false,
        }
    }
    pub fn is_empty(&self) -> bool {
        matches!(self, Node::Empty)
    }

    pub fn children(&self) -> anyhow::Result<(Vec<u8>, Vec<u8>)> {
        match self {
            Node::BranchNode { right, left } => Ok((left.clone(), right.clone())),
            _ => Err(anyhow::anyhow!("Invalid function call")),
        }
    }
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

    pub fn digest_node(&self, node: &Node) -> Vec<u8> {
        match node {
            Node::BranchNode { right, left } => {
                let mut data = Self::NODE_PREFIX.to_vec();
                data.extend_from_slice(right);
                data.extend_from_slice(left);
                self.hasher.hash(&data).into()
            }
            Node::LeafNode { path, value } => {
                let mut data = Self::LEAF_PREFIX.to_vec();
                data.extend_from_slice(path);
                data.extend_from_slice(value);
                self.hasher.hash(&data).into()
            }
            Node::Empty => {
                vec![]
            }
        }
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

    pub fn sidenodes(&self, root: &[u8], path: &[u8]) -> anyhow::Result<(Vec<Node>, Vec<Node>)> {
        let mut sidenodes = Vec::<Node>::new();
        let mut pathnodes = Vec::<Node>::new();

        if root == self.tree_hasher.zero_hash {
            return Ok((sidenodes, pathnodes));
        }

        // Node corresponding to root hash should exist
        let mut node: Node = self.nodes.get(root).as_slice().try_into()?;

        let mut pathnode: Node;
        let mut sidenode: Node;

        for p in path.iter().take(self.depth()) {
            if node.is_leaf() || node.is_empty() {
                // leaf nodes do not have sidenodes
                return Ok((sidenodes, pathnodes));
            }

            let (left, right) = node.children()?;
            if *p == 0 {
                // 0 is left traversal
                pathnode = left.as_slice().try_into()?;
                sidenode = right.as_slice().try_into()?;
            } else {
                // 1 is right traversal
                pathnode = right.as_slice().try_into()?;
                sidenode = left.as_slice().try_into()?;
            }

            if !sidenode.is_empty() {
                sidenodes.push(sidenode);
            }

            if !pathnode.is_empty() {
                pathnodes.push(pathnode.clone());
            }

            node = pathnode;
        }

        Ok((sidenodes, pathnodes))
    }

    fn depth(&self) -> usize {
        self.tree_hasher.hasher.output_size()
    }
}

fn main() {
    println!("Hello, world!");
}
