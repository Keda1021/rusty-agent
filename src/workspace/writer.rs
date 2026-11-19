use std::error::Error;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

pub const MAX_WRITE_BYTES: usize = 64_000;

pub(crate) fn resolve_writable_path(
    workspace: &Path,
    raw_path: &str,
) -> Result<PathBuf, Box<dyn Error>> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err("Đường dẫn file không được để trống".into());
    }

    let canonical_root = fs::canonicalize(workspace)?;
    let target = workspace.join(trimmed);

    if target.exists() {
        let canonical_target = fs::canonicalize(&target)?;
        if !canonical_target.starts_with(&canonical_root) {
            return Err("Đường dẫn nằm ngoài workspace cho phép".into());
        }
        if canonical_target.is_dir() {
            return Err("Đường dẫn trỏ tới thư mục, không thể ghi đè như file".into());
        }
        return Ok(canonical_target);
    }

    let mut ancestor = target.parent();
    let mut missing_parts: Vec<std::ffi::OsString> = Vec::new();

    if let Some(file_name) = target.file_name() {
        missing_parts.push(file_name.to_os_string());
    }

    let canonical_existing_parent = loop {
        match ancestor {
            Some(p) if p.exists() => {
                let canon = fs::canonicalize(p)?;
                if !canon.starts_with(&canonical_root) {
                    return Err("Đường dẫn nằm ngoài workspace cho phép".into());
                }
                break canon;
            }
            Some(p) => {
                if let Some(name) = p.file_name() {
                    missing_parts.push(name.to_os_string());
                }
                ancestor = p.parent();
            }
            None => return Err("Không thể xác định thư mục cha hợp lệ".into()),
        }
    };

    let mut final_path = canonical_existing_parent;
    for part in missing_parts.into_iter().rev() {
        final_path.push(part);
    }

    if !final_path.starts_with(&canonical_root) {
        return Err("Đường dẫn nằm ngoài workspace cho phép".into());
    }

    Ok(final_path)
}
fn confirm_write<I>(target: &Path, existed: bool, content: &str, lines: &mut I) -> io::Result<bool>
where
    I: Iterator<Item = io::Result<String>>,
{
    let line_count = content.lines().count();
    let byte_count = content.len();
    let action = if existed { "GHI ĐÈ" } else { "TẠO MỚI" };

    println!("\n=== XÁC NHẬN GHI FILE ({}) ===", action);
    println!("File : {}", target.display());
    println!("Kích thước: {} bytes | {} dòng", byte_count, line_count);
    println!("--- Xem trước nội dung ---");
    for (idx, line) in content.lines().take(20).enumerate() {
        println!("{:>3} | {}", idx + 1, line);
    }
    if line_count > 20 {
        println!("... (còn {} dòng nữa)", line_count - 20);
    }
    println!("---------------------------");
    print!("Lưu thay đổi này? [y/N]: ");
    io::stdout().flush()?;

    let answer = match lines.next() {
        Some(Ok(s)) => s,
        Some(Err(e)) => return Err(e),
        None => String::new(),
    };
    let normalized = answer.trim().to_lowercase();

    Ok(matches!(normalized.as_str(), "y" | "yes"))
}

pub fn write_file_with_confirm<I>(
    workspace: &Path,
    raw_path: &str,
    content: &str,
    lines: &mut I,
) -> Result<bool, Box<dyn Error>>
where
    I: Iterator<Item = io::Result<String>>,
{
    if content.len() > MAX_WRITE_BYTES {
        return Err(format!(
            "Nội dung quá lớn ({} bytes > giới hạn {} bytes)",
            content.len(),
            MAX_WRITE_BYTES
        )
        .into());
    }

    let target = resolve_writable_path(workspace, raw_path)?;
    let existed = target.exists();

    if !confirm_write(&target, existed, content, lines)? {
        println!("Đã hủy thao tác ghi file.");
        return Ok(false);
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&target, content)?;
    println!("Đã lưu file thành công: {}", target.display());
    Ok(true)
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
        let dir = std::env::temp_dir().join(format!("rusty_agent_writer_{}_{}", label, nanos));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn resolves_nested_nonexistent_file_inside_workspace() {
        let ws = make_temp_dir("nested_new");
        let resolved = resolve_writable_path(&ws, "notes/2026/plan.md").unwrap();

        let canon_ws = fs::canonicalize(&ws).unwrap();
        assert!(resolved.starts_with(&canon_ws));
        assert!(resolved.ends_with(Path::new("notes/2026/plan.md")));
        assert!(!resolved.exists());

        let _ = fs::remove_dir_all(ws);
    }

    #[test]
    fn resolves_existing_file_inside_workspace() {
        let ws = make_temp_dir("existing");
        let file_path = ws.join("sample.txt");
        fs::write(&file_path, "hello").unwrap();

        let resolved = resolve_writable_path(&ws, "sample.txt").unwrap();
        assert_eq!(resolved, fs::canonicalize(&file_path).unwrap());

        let _ = fs::remove_dir_all(ws);
    }

    #[test]
    fn rejects_traversal_outside_workspace() {
        let ws = make_temp_dir("escape");
        let err = resolve_writable_path(&ws, "../outside_secret.txt").unwrap_err();
        assert!(err.to_string().contains("ngoài workspace"));

        let _ = fs::remove_dir_all(ws);
    }

    #[test]
    fn rejects_writing_over_directory() {
        let ws = make_temp_dir("dir_target");
        fs::create_dir_all(ws.join("sub")).unwrap();

        let err = resolve_writable_path(&ws, "sub").unwrap_err();
        assert!(err.to_string().contains("thư mục"));

        let _ = fs::remove_dir_all(ws);
    }
}
