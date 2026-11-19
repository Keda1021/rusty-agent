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
