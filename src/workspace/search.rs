use super::explorer::should_skip_dir;
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug)]
pub struct SearchHit {
    pub file: String,
    pub line: usize,
    pub snippet: String,
}

fn is_searchable_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str(),
        "rs" | "toml"
            | "md"
            | "txt"
            | "json"
            | "yaml"
            | "yml"
            | "js"
            | "ts"
            | "py"
            | "go"
            | "c"
            | "cpp"
            | "h"
    )
}

pub fn search_workspace(
    root: &Path,
    dir: &Path,
    query: &str,
    max_hits: usize,
    hits: &mut Vec<SearchHit>,
) -> io::Result<()> {
    if hits.len() >= max_hits {
        return Ok(());
    }

    let needle = query.to_lowercase();
    for entry in fs::read_dir(dir)?.filter_map(Result::ok) {
        if hits.len() >= max_hits {
            break;
        }

        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

        if is_dir {
            if !should_skip_dir(&name) {
                search_workspace(root, &path, query, max_hits, hits)?;
            }
        } else if is_searchable_file(&path) {
            if let Ok(content) = fs::read_to_string(&path) {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                for (idx, line) in content.lines().enumerate() {
                    if line.to_lowercase().contains(&needle) {
                        hits.push(SearchHit {
                            file: rel.clone(),
                            line: idx + 1,
                            snippet: line.trim().to_string(),
                        });
                        if hits.len() >= max_hits {
                            break;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("rusty_agent_search_{}_{}", label, nanos));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn finds_case_insensitive_matches_and_skips_ignored_dirs() {
        let ws = make_temp_dir("find_basic");
        fs::create_dir_all(ws.join("src")).unwrap();
        fs::create_dir_all(ws.join("target/debug")).unwrap();

        fs::write(ws.join("src/lib.rs"), "pub fn run_agent() {}\nlet x = 1;\n").unwrap();
        fs::write(ws.join("README.md"), "Guide for RUN_AGENT tool\n").unwrap();
        fs::write(ws.join("target/debug/cached.rs"), "pub fn run_agent() {}\n").unwrap();

        let mut hits = Vec::new();
        search_workspace(&ws, &ws, "run_agent", 20, &mut hits).unwrap();

        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|h| !h.file.contains("target")));
        assert!(hits
            .iter()
            .any(|h| h.line == 1 && h.snippet.contains("run_agent")));

        let _ = fs::remove_dir_all(ws);
    }

    #[test]
    fn respects_max_hits_limit() {
        let ws = make_temp_dir("max_hits");
        fs::write(
            ws.join("notes.txt"),
            "match one\nmatch two\nmatch three\nmatch four\n",
        )
        .unwrap();

        let mut hits = Vec::new();
        search_workspace(&ws, &ws, "match", 2, &mut hits).unwrap();

        assert_eq!(hits.len(), 2);

        let _ = fs::remove_dir_all(ws);
    }
}
