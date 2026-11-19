use std::error::Error;
use std::fs;
use std::path::Path;

const IGNORED_DIRS: &[&str] = &[".git", "target", "node_modules", ".idea", ".vscode"];
const SEARCHABLE_EXTS: &[&str] = &[
    "rs", "toml", "json", "yaml", "yml", "md", "txt", "js", "ts", "tsx", "jsx", "py", "go", "c",
    "cpp", "h", "hpp", "java", "sh", "ps1", "sql", "html", "css",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub file: String,
    pub line: usize,
    pub snippet: String,
}

fn is_ignored_dir(name: &str) -> bool {
    name.starts_with('.') || IGNORED_DIRS.contains(&name)
}

fn is_searchable_file(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    SEARCHABLE_EXTS.contains(&ext.to_lowercase().as_str())
}

pub fn search_workspace(
    root: &Path,
    current_dir: &Path,
    query: &str,
    max_hits: usize,
    hits: &mut Vec<SearchHit>,
) -> Result<(), Box<dyn Error>> {
    if query.trim().is_empty() || max_hits == 0 || hits.len() >= max_hits {
        return Ok(());
    }

    let needle_lower = query.to_lowercase();

    for entry in fs::read_dir(current_dir)? {
        if hits.len() >= max_hits {
            return Ok(());
        }

        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() {
            if is_ignored_dir(&name) {
                continue;
            }
            search_workspace(root, &path, query, max_hits, hits)?;
        } else if is_searchable_file(&path)
            && let Ok(content) = fs::read_to_string(&path)
        {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");

            for (idx, line) in content.lines().enumerate() {
                if line.to_lowercase().contains(&needle_lower) {
                    hits.push(SearchHit {
                        file: rel.clone(),
                        line: idx + 1,
                        snippet: line.trim().to_string(),
                    });
                    if hits.len() >= max_hits {
                        return Ok(());
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
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir(label: &str) -> std::path::PathBuf {
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
        let ws = make_temp_dir("matches");
        fs::create_dir_all(ws.join("src")).unwrap();
        fs::create_dir_all(ws.join("target")).unwrap();

        fs::write(
            ws.join("src/main.rs"),
            "fn run_agent() {}\nlet target_name = \"demo\";\n",
        )
        .unwrap();
        fs::write(ws.join("target/ignored.rs"), "fn run_agent() {}\n").unwrap();

        let mut hits = Vec::new();
        search_workspace(&ws, &ws, "RUN_AGENT", 20, &mut hits).unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].file, "src/main.rs");
        assert_eq!(hits[0].line, 1);
        assert!(hits[0].snippet.contains("run_agent"));

        let _ = fs::remove_dir_all(ws);
    }

    #[test]
    fn respects_max_hits_limit() {
        let ws = make_temp_dir("limit");
        fs::write(ws.join("notes.md"), "alpha one\nalpha two\nalpha three\n").unwrap();

        let mut hits = Vec::new();
        search_workspace(&ws, &ws, "alpha", 2, &mut hits).unwrap();

        assert_eq!(hits.len(), 2);

        let _ = fs::remove_dir_all(ws);
    }
}
