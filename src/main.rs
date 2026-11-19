mod llm;
mod workspace;

use llm::{Message, OllamaClient, DEFAULT_MODEL};
use serde_json::Value;
use std::error::Error;
use std::io::{self, BufRead, Write};
use workspace::{
    list_tree, read_file, resolve_workspace_path, search_workspace, write_file_with_confirm,
};

const SYSTEM_PROMPT: &str = "Bạn là trợ lý phân tích mã nguồn trong workspace Rust. Khi cần thông tin về cấu trúc thư mục, nội dung file hoặc vị trí từ khóa trong dự án, hãy gọi tool phù hợp trước khi kết luận. Trả lời rõ ràng, súc tích, bằng tiếng Việt khi người dùng hỏi tiếng Việt.";

fn new_history() -> Vec<Message> {
    vec![Message {
        role: "system".to_string(),
        content: SYSTEM_PROMPT.to_string(),
        tool_calls: None,
    }]
}

fn print_help() {
    println!("Các lệnh hỗ trợ:");
    println!("  /ls [path]               - Liệt kê cây thư mục");
    println!("  /read <file>             - Xem nội dung file");
    println!("  /find <từ_khóa>          - Tìm chuỗi trong các file của workspace");
    println!("  /write <file> <nội_dung> - Ghi/cập nhật file có xác nhận [y/N]");
    println!("  /write-block <file>      - Nhập nhiều dòng (kết thúc bằng ::EOF) và xác nhận lưu");
    println!("  /ask <file> <câu_hỏi>    - Gửi nội dung file kèm câu hỏi cho LLM");
    println!("  /model [tên_model]       - Xem hoặc đổi model Ollama hiện tại");
    println!("  /clear                   - Xóa lịch sử hội thoại để nhẹ context");
    println!("  /help                    - Hiển thị hướng dẫn");
    println!("  /exit                    - Thoát chương trình");
    println!("  (Hoặc gõ câu hỏi thường để agent tự gọi tool khám phá workspace)");
}

fn execute_workspace_tool(workspace: &std::path::Path, name: &str, args: &Value) -> String {
    match name {
        "list_tree" => {
            let rel = args.get("path").and_then(Value::as_str).unwrap_or(".");
            let max_depth = args.get("max_depth").and_then(Value::as_u64).unwrap_or(3) as usize;

            match resolve_workspace_path(workspace, rel) {
                Ok(dir) => {
                    let mut tree = format!("{}/\n", dir.display());
                    if let Err(e) = list_tree(workspace, &dir, "", 0, max_depth, &mut tree) {
                        format!("Lỗi list_tree: {}", e)
                    } else {
                        tree
                    }
                }
                Err(e) => format!("Lỗi đường dẫn: {}", e),
            }
        }
        "read_file" => match args.get("path").and_then(Value::as_str) {
            Some(path) => {
                read_file(workspace, path).unwrap_or_else(|e| format!("Lỗi read_file: {}", e))
            }
            None => "Thiếu tham số bắt buộc 'path'".to_string(),
        },
        "search_workspace" => match args.get("query").and_then(Value::as_str) {
            Some(query) => {
                let max_hits = args.get("max_hits").and_then(Value::as_u64).unwrap_or(20) as usize;
                let mut hits = Vec::new();
                match search_workspace(workspace, workspace, query, max_hits, &mut hits) {
                    Ok(()) => {
                        if hits.is_empty() {
                            format!("Không tìm thấy từ khóa '{}'", query)
                        } else {
                            hits.into_iter()
                                .map(|h| format!("{}:{}: {}", h.file, h.line, h.snippet))
                                .collect::<Vec<_>>()
                                .join("\n")
                        }
                    }
                    Err(e) => format!("Lỗi search_workspace: {}", e),
                }
            }
            None => "Thiếu tham số bắt buộc 'query'".to_string(),
        },
        other => format!("Tool không được hỗ trợ: {}", other),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let workspace = std::env::current_dir()?;
    let mut client = OllamaClient::new(DEFAULT_MODEL)?;
    let mut history = new_history();

    println!("Workspace Explorer đang chạy tại: {}", workspace.display());
    println!("Model hiện tại: {}", client.model);
    print_help();

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    while let Some(line_res) = lines.next() {
        let input = line_res?;
        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "/exit" {
            break;
        }
        if trimmed == "/help" {
            print_help();
            continue;
        }
        if trimmed == "/clear" {
            history = new_history();
            println!("Đã xóa lịch sử hội thoại.");
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("/model") {
            let name = rest.trim();
            if name.is_empty() {
                println!("Model hiện tại: {}", client.model);
            } else {
                client.set_model(name);
                println!("Đã chuyển sang model: {}", client.model);
            }
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("/ls") {
            let sub = rest.trim();
            let target_dir = if sub.is_empty() {
                workspace.clone()
            } else {
                match resolve_workspace_path(&workspace, sub) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("Lỗi đường dẫn: {}", e);
                        continue;
                    }
                }
            };
            let mut tree = format!("{}/\n", target_dir.display());
            if let Err(e) = list_tree(&workspace, &target_dir, "", 0, 3, &mut tree) {
                eprintln!("Lỗi duyệt thư mục: {}", e);
            } else {
                println!("{}", tree);
            }
            continue;
        }

        if let Some(file_path) = trimmed.strip_prefix("/read ") {
            match read_file(&workspace, file_path) {
                Ok(content) => println!("--- {} ---\n{}\n", file_path.trim(), content),
                Err(e) => eprintln!("Lỗi đọc file: {}", e),
            }
            continue;
        }

        if let Some(query) = trimmed.strip_prefix("/find ") {
            let q = query.trim();
            if q.is_empty() {
                eprintln!("Cú pháp: /find <từ_khóa>");
                continue;
            }
            let mut hits = Vec::new();
            if let Err(e) = search_workspace(&workspace, &workspace, q, 30, &mut hits) {
                eprintln!("Lỗi tìm kiếm: {}", e);
            } else if hits.is_empty() {
                println!("Không tìm thấy từ khóa '{}'.", q);
            } else {
                for hit in hits {
                    println!("{}:{}: {}", hit.file, hit.line, hit.snippet);
                }
            }
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("/write ") {
            let mut parts = rest.trim().splitn(2, char::is_whitespace);
            let file_path = parts.next().unwrap_or("");
            let content = parts.next().unwrap_or("");

            if file_path.is_empty() || content.is_empty() {
                eprintln!("Cú pháp: /write <đường_dẫn_file> <nội_dung>");
                continue;
            }

            if let Err(e) = write_file_with_confirm(&workspace, file_path, content, &mut lines) {
                eprintln!("Lỗi ghi file: {}", e);
            }
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("/write-block ") {
            let file_path = rest.trim();
            if file_path.is_empty() {
                eprintln!("Cú pháp: /write-block <đường_dẫn_file>");
                continue;
            }

            println!(
                "Nhập nội dung cho `{}`. Gõ `::EOF` trên một dòng riêng để kết thúc:",
                file_path
            );
            let mut block = String::new();
            while let Some(body_line) = lines.next() {
                let raw = body_line?;
                if raw.trim() == "::EOF" {
                    break;
                }
                block.push_str(&raw);
                block.push('\n');
            }

            if let Err(e) = write_file_with_confirm(&workspace, file_path, &block, &mut lines) {
                eprintln!("Lỗi ghi file: {}", e);
            }
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("/ask ") {
            let mut parts = rest.trim().splitn(2, char::is_whitespace);
            let file_path = parts.next().unwrap_or("");
            let question = parts.next().unwrap_or("").trim();

            if file_path.is_empty() || question.is_empty() {
                eprintln!("Cú pháp: /ask <đường_dẫn_file> <câu_hỏi>");
                continue;
            }

            let content = match read_file(&workspace, file_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Lỗi đọc file: {}", e);
                    continue;
                }
            };

            history.push(Message {
                role: "user".to_string(),
                content: format!(
                    "Dưới đây là nội dung file `{}`:\n```\n{}\n```\n\nCâu hỏi: {}",
                    file_path, content, question
                ),
                tool_calls: None,
            });

            print!("[{}]: ", client.model);
            io::stdout().flush()?;
            if let Err(e) = client.send_with_tools(&mut history, |name, args| {
                execute_workspace_tool(&workspace, name, args)
            }) {
                eprintln!("\nLỗi gọi Ollama: {}", e);
                history.pop();
            }
            continue;
        }

        history.push(Message {
            role: "user".to_string(),
            content: trimmed.to_string(),
            tool_calls: None,
        });

        print!("[{}]: ", client.model);
        io::stdout().flush()?;
        if let Err(e) = client.send_with_tools(&mut history, |name, args| {
            execute_workspace_tool(&workspace, name, args)
        }) {
            eprintln!("\nLỗi gọi Ollama: {}", e);
            history.pop();
        }
    }

    Ok(())
}
