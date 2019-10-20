use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub children: HashMap<String, Box<Node>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tree {
    pub node: Box<Node>,
}

impl Default for Tree {
    fn default() -> Tree {
        Tree::new()
    }
}

impl Tree {
    pub fn new() -> Tree {
        Tree {
            node: Box::new(Node::new()),
        }
    }

    pub fn get_path(&self, path: &str) -> Option<&Node> {
        let mut node = &self.node;
        let split_path: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        for path_elem in split_path {
            if !node.children.contains_key(path_elem) {
                return None;
            }
            node = node.children.get(path_elem).unwrap()
        }
        Some(&*node)
    }

    pub fn add_path(&mut self, path: &str) {
        let mut node = &mut self.node;
        let split_path: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        for path_elem in split_path {
            if !node.children.contains_key(path_elem) {
                node.insert(path_elem);
            }
            node = node.children.get_mut(path_elem).unwrap()
        }
    }
}

impl Node {
    pub fn new() -> Node {
        Node {
            children: HashMap::new(),
        }
    }

    pub fn insert(&mut self, name: &str) {
        let new_node = Node::new();
        self.children.insert(name.to_string(), Box::new(new_node));
    }

    pub fn sorted_childrens(&self) -> Vec<String> {
        let mut dirs: std::vec::Vec<String> =
            self.children.iter().map(|(k, _)| k.clone()).collect();
        dirs.sort();
        dirs
    }
}
