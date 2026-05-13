use crate::RbmemDocument;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::Path;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SectionIndex {
    keywords: BTreeMap<String, BTreeSet<String>>,
    paths: Vec<String>,
    adjacency: BTreeMap<String, BTreeSet<String>>,
    trie: PathTrie,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
struct PathTrie {
    terminal: bool,
    children: BTreeMap<String, PathTrie>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedSectionIndex {
    pub modified_at: SystemTime,
    pub index: SectionIndex,
}

impl SectionIndex {
    pub fn build(document: &RbmemDocument) -> Self {
        let mut keywords: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut paths = Vec::new();
        let mut trie = PathTrie::default();
        let mut adjacency: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

        for section in &document.sections {
            paths.push(section.path.clone());
            trie.insert(&section.path);
            for token in tokenize(&format!("{} {}", section.path, section.content)) {
                keywords
                    .entry(token)
                    .or_default()
                    .insert(section.path.clone());
            }
        }

        for edge in document.graph_view().edges {
            adjacency
                .entry(edge.from.clone())
                .or_default()
                .insert(edge.to.clone());
            adjacency.entry(edge.to).or_default().insert(edge.from);
        }

        paths.sort();
        Self {
            keywords,
            paths,
            adjacency,
            trie,
        }
    }

    pub fn keyword(&self, keyword: &str) -> Vec<String> {
        self.keywords
            .get(&normalize_token(keyword))
            .into_iter()
            .flat_map(|paths| paths.iter().cloned())
            .collect()
    }

    pub fn prefix(&self, pattern: &str) -> Vec<String> {
        let prefix = pattern.strip_suffix(".*").unwrap_or(pattern);
        self.trie.prefix(prefix)
    }

    pub fn related(&self, path: &str, depth: usize) -> Vec<String> {
        if depth == 0 {
            return Vec::new();
        }

        let mut visited = BTreeSet::from([path.to_string()]);
        let mut frontier = VecDeque::from([(path.to_string(), 0usize)]);
        let mut related = BTreeSet::new();

        while let Some((current, current_depth)) = frontier.pop_front() {
            if current_depth >= depth {
                continue;
            }
            if let Some(neighbors) = self.adjacency.get(&current) {
                for neighbor in neighbors {
                    if self.paths.binary_search(neighbor).is_err() {
                        continue;
                    }
                    if visited.insert(neighbor.clone()) {
                        related.insert(neighbor.clone());
                        frontier.push_back((neighbor.clone(), current_depth + 1));
                    }
                }
            }
        }

        related.into_iter().collect()
    }

    pub fn save_disk_cache(&self, path: impl AsRef<Path>) -> Result<(), crate::RbmemError> {
        fs::write(path, serde_json::to_string(self)?)?;
        Ok(())
    }

    pub fn load_disk_cache(path: impl AsRef<Path>) -> Result<Self, crate::RbmemError> {
        Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
    }
}

impl CachedSectionIndex {
    pub fn load_or_rebuild(
        memory_path: impl AsRef<Path>,
        document: &RbmemDocument,
    ) -> Result<Self, crate::RbmemError> {
        let memory_path = memory_path.as_ref();
        let modified_at = fs::metadata(memory_path)?.modified()?;
        Ok(Self {
            modified_at,
            index: SectionIndex::build(document),
        })
    }

    pub fn is_valid_for(&self, memory_path: impl AsRef<Path>) -> Result<bool, crate::RbmemError> {
        Ok(fs::metadata(memory_path)?.modified()? == self.modified_at)
    }
}

impl PathTrie {
    fn insert(&mut self, path: &str) {
        let mut node = self;
        for segment in path.split('.') {
            node = node.children.entry(segment.to_string()).or_default();
        }
        node.terminal = true;
    }

    fn prefix(&self, prefix: &str) -> Vec<String> {
        let mut node = self;
        for segment in prefix.split('.').filter(|segment| !segment.is_empty()) {
            let Some(next) = node.children.get(segment) else {
                return Vec::new();
            };
            node = next;
        }

        let mut output = Vec::new();
        node.collect(prefix.trim_end_matches('.').to_string(), &mut output);
        output.sort();
        output
    }

    fn collect(&self, prefix: String, output: &mut Vec<String>) {
        if self.terminal {
            output.push(prefix.clone());
        }
        for (segment, child) in &self.children {
            let path = if prefix.is_empty() {
                segment.clone()
            } else {
                format!("{prefix}.{segment}")
            };
            child.collect(path, output);
        }
    }
}

fn tokenize(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(normalize_token)
        .filter(|token| token.len() > 1)
}

fn normalize_token(token: &str) -> String {
    token.trim().to_ascii_lowercase()
}
