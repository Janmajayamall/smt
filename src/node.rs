use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, PartialEq, Clone)]
pub enum Node {
    InternalNode { right: Vec<u8>, left: Vec<u8> },
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

    pub fn is_branch(&self) -> bool {
        match self {
            Node::InternalNode { right: _, left: _ } => true,
            _ => false,
        }
    }

    pub fn children(&self) -> (Vec<u8>, Vec<u8>) {
        match self {
            Node::InternalNode { right, left } => (left.clone(), right.clone()),
            _ => (vec![], vec![]),
        }
    }

    pub fn match_leaf_path(&self, match_path: &[u8]) -> bool {
        match self {
            Node::LeafNode { path, value: _ } => path == match_path,
            _ => false,
        }
    }
}
