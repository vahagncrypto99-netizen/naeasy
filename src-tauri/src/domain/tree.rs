use std::fs;
use std::path::{Path, PathBuf};

use super::models::{default_shortcut, AppData, Config, TreeNode, Workspace, WorkspaceView};

const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "vendor",
    "storage",
    "dist",
    "build",
    "target",
    ".next",
    ".cache",
];

const MAX_DEPTH: usize = 6;

/// Recursively collect directories that contain a `.git` entry.
/// Does not descend into a repo once found (repos are leaves).
pub fn find_repos(root: &Path) -> Vec<PathBuf> {
    let mut repos = Vec::new();
    walk(root, 0, &mut repos);
    repos.sort();
    repos
}

fn walk(dir: &Path, depth: usize, repos: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH {
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut subdirs: Vec<PathBuf> = Vec::new();
    let mut is_repo = false;

    for entry in entries.flatten() {
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" {
            is_repo = true;
            // keep scanning entries to finish, but we won't descend
        }
        subdirs.push(entry.path());
    }

    if is_repo {
        repos.push(dir.to_path_buf());
        return; // a repo is a leaf — do not descend
    }

    for sub in subdirs {
        let name = sub
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if name.starts_with('.') {
            continue;
        }
        if SKIP_DIRS.contains(&name.as_str()) {
            continue;
        }
        walk(&sub, depth + 1, repos);
    }
}

/// Build a nested tree from repo paths relative to `root`.
pub fn build_tree(root: &Path, repos: &[PathBuf]) -> TreeNode {
    let mut tree = TreeNode {
        name: root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| root.to_string_lossy().to_string()),
        path: root.to_string_lossy().to_string(),
        is_repo: false,
        children: Vec::new(),
    };

    for repo in repos {
        let rel = match repo.strip_prefix(root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let parts: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();

        let mut node = &mut tree;
        let mut acc = root.to_path_buf();
        let last = parts.len().saturating_sub(1);
        for (i, part) in parts.iter().enumerate() {
            acc = acc.join(part);
            let idx = match node.children.iter().position(|c| &c.name == part) {
                Some(idx) => idx,
                None => {
                    node.children.push(TreeNode {
                        name: part.clone(),
                        path: acc.to_string_lossy().to_string(),
                        is_repo: false,
                        children: Vec::new(),
                    });
                    node.children.len() - 1
                }
            };
            node = &mut node.children[idx];
            if i == last {
                node.is_repo = true;
            }
        }
    }

    sort_tree(&mut tree);
    tree
}

fn sort_tree(node: &mut TreeNode) {
    // Folders (groups) first, then repos, each alphabetical.
    node.children.sort_by(|a, b| {
        let a_group = !a.children.is_empty();
        let b_group = !b.children.is_empty();
        b_group
            .cmp(&a_group)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    for child in &mut node.children {
        sort_tree(child);
    }
}

pub fn count_repos(node: &TreeNode) -> usize {
    let mut n = if node.is_repo { 1 } else { 0 };
    for c in &node.children {
        n += count_repos(c);
    }
    n
}

pub fn build_view(ws: &Workspace) -> WorkspaceView {
    let root = PathBuf::from(&ws.path);
    let repos = find_repos(&root);
    let tree = build_tree(&root, &repos);
    let repo_count = count_repos(&tree);
    WorkspaceView {
        id: ws.id.clone(),
        name: ws.name.clone(),
        path: ws.path.clone(),
        tree,
        repo_count,
    }
}

pub fn build_app_data(config: &Config) -> AppData {
    AppData {
        workspaces: config.workspaces.iter().map(build_view).collect(),
        ides: config.ides.clone(),
        default_ide_id: config.default_ide_id.clone(),
        shortcut: config.shortcut.clone().unwrap_or_else(default_shortcut),
        recents: config.recents.clone(),
        project_ides: config.project_ides.clone(),
        open_in_tabs: config.open_in_tabs,
        base_branches: config.base_branches.clone(),
        default_base_branch: config.default_base_branch.clone(),
        project_base_branches: config.project_base_branches.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn make_repo(base: &Path, rel: &str) {
        fs::create_dir_all(base.join(rel).join(".git")).unwrap();
    }

    fn rel_names(root: &Path, repos: &[PathBuf]) -> Vec<String> {
        repos
            .iter()
            .map(|p| {
                p.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect()
    }

    #[test]
    fn finds_repos_and_does_not_descend_into_them() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_repo(root, "pc");
        make_repo(root, "adapters/foo");
        make_repo(root, "adapters/bar");
        // A repo nested inside another repo must NOT be discovered.
        make_repo(root, "pc/sub/inner");

        let names = rel_names(root, &find_repos(root));
        assert!(names.contains(&"pc".to_string()));
        assert!(names.contains(&"adapters/foo".to_string()));
        assert!(names.contains(&"adapters/bar".to_string()));
        assert!(!names.iter().any(|n| n.contains("inner")));
        assert_eq!(names.len(), 3);
    }

    #[test]
    fn skips_noise_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_repo(root, "node_modules/pkg");
        make_repo(root, "vendor/lib");
        make_repo(root, "real");

        let names = rel_names(root, &find_repos(root));
        assert_eq!(names, vec!["real".to_string()]);
    }

    #[test]
    fn builds_grouped_sorted_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        make_repo(root, "adapters/foo");
        make_repo(root, "adapters/bar");
        make_repo(root, "pc");

        let tree = build_tree(root, &find_repos(root));
        assert_eq!(count_repos(&tree), 3);

        // Groups come before repos.
        assert_eq!(tree.children[0].name, "adapters");
        assert!(!tree.children[0].is_repo);
        assert_eq!(tree.children[0].children.len(), 2);
        // Children are alphabetical.
        assert_eq!(tree.children[0].children[0].name, "bar");
        assert_eq!(tree.children[0].children[1].name, "foo");

        let pc = tree.children.iter().find(|c| c.name == "pc").unwrap();
        assert!(pc.is_repo);
    }
}
