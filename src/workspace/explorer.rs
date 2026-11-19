use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn should_skip_dir(name: &str) -> bool {
    matches!(
        name,
        "target" | ".git" | "node_modules" | ".idea" | ".vscode"
    )
}

pub fn resolve_workspace_path(workspace: &Path, raw_path: &str) -> Result<PathBuf, Box<dyn Error>> {
    let canonical_root = fs::canonicalize(workspace)?;
    let target = workspace.join(raw_path.trim());
    let canonical_target = fs::canonicalize(&target)?;

    if !canonical_target.starts_with(&canonical_root) {
        return Err("Đường dẫn nằm ngoài workspace cho phép".into());
    }
    Ok(canonical_target)
}

pub fn list_tree(
    root: &Path,
    dir: &Path,
    prefix: &str,
    depth: usize,
    max_depth: usize,
    out: &mut String,
) -> io::Result<()> {
    if depth > max_depth {
        return Ok(());
    }

    let mut entries: Vec<_> = fs::read_dir(dir)?.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| {
        (
            e.file_type().map(|t| !t.is_dir()).unwrap_or(true),
            e.file_name(),
        )
    });

    let total = entries.len();
    for (idx, entry) in entries.into_iter().enumerate() {
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

        if is_dir && should_skip_dir(&name) {
            continue;
        }

        let is_last = idx + 1 == total;
        let connector = if is_last { "└── " } else { "├── " };
        let rel = entry
            .path()
            .strip_prefix(root)
            .unwrap_or(&entry.path())
            .display()
            .to_string();

        if is_dir {
            out.push_str(&format!("{}{}{}/\n", prefix, connector, name));
            let next_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
            list_tree(root, &entry.path(), &next_prefix, depth + 1, max_depth, out)?;
        } else {
            out.push_str(&format!("{}{}{} ({})\n", prefix, connector, name, rel));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("rusty_agent_explorer_{}_{}", label, nanos));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn lists_tree_and_skips_ignored_dirs() {
        let ws = make_temp_dir("tree");
        fs::create_dir_all(ws.join("src")).unwrap();
        fs::create_dir_all(ws.join("target")).unwrap();
        fs::write(ws.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(ws.join("target/ignored.txt"), "skip").unwrap();

        let mut out = String::new();
        list_tree(&ws, &ws, "", 0, 2, &mut out).unwrap();

        assert!(out.contains("src/"));
        assert!(out.contains("main.rs"));
        assert!(!out.contains("target/"));
        assert!(!out.contains("ignored.txt"));

        let _ = fs::remove_dir_all(ws);
    }

    #[test]
    fn rejects_path_outside_workspace() {
        let ws = make_temp_dir("escape");
        let err = resolve_workspace_path(&ws, "..").unwrap_err();
        assert!(err.to_string().contains("ngoài workspace"));

        let _ = fs::remove_dir_all(ws);
    }
}
