use super::explorer::resolve_workspace_path;
use std::error::Error;
use std::fs;
use std::path::Path;

pub const MAX_FILE_BYTES: usize = 24_000;

pub fn read_file(workspace: &Path, raw_path: &str) -> Result<String, Box<dyn Error>> {
    let path = resolve_workspace_path(workspace, raw_path)?;
    if path.is_dir() {
        return Err("Đây là thư mục, hãy dùng /ls thay vì /read".into());
    }

    let bytes = fs::read(&path)?;
    if bytes.len() > MAX_FILE_BYTES {
        let truncated = String::from_utf8_lossy(&bytes[..MAX_FILE_BYTES]);
        return Ok(format!(
            "[Cảnh báo: File lớn ({} bytes), chỉ đọc {} bytes đầu]\n\n{}",
            bytes.len(),
            MAX_FILE_BYTES,
            truncated
        ));
    }

    Ok(String::from_utf8_lossy(&bytes).into_owned())
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
        let dir = std::env::temp_dir().join(format!("rusty_agent_reader_{}_{}", label, nanos));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_file_inside_workspace() {
        let ws = make_temp_dir("read_ok");
        fs::write(ws.join("sample.txt"), "noi dung test").unwrap();

        let content = read_file(&ws, "sample.txt").unwrap();
        assert_eq!(content, "noi dung test");

        let _ = fs::remove_dir_all(ws);
    }

    #[test]
    fn rejects_directory_read() {
        let ws = make_temp_dir("read_dir");
        fs::create_dir_all(ws.join("folder")).unwrap();

        let err = read_file(&ws, "folder").unwrap_err();
        assert!(err.to_string().contains("thư mục"));

        let _ = fs::remove_dir_all(ws);
    }
}
