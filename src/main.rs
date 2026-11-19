mod llm;
mod workspace;

use llm::{build_client, send_chat, Message};
use std::error::Error;
use std::io::{self, BufRead, Write};
use workspace::{list_tree, read_file, resolve_workspace_path};

fn main() -> Result<(), Box<dyn Error>> {
    let workspace = std::env::current_dir()?;
    let client = build_client()?;

    let mut history: Vec<Message> = vec![Message {
        role: "system".to_string(),
        content: "Bạn là trợ lý phân tích mã nguồn trong workspace Rust. Trả lời rõ ràng, súc tích, bằng tiếng Việt khi người dùng hỏi tiếng Việt.".to_string(),
    }];

    println!("Workspace Explorer đang chạy tại: {}", workspace.display());
    println!("Lệnh: /ls [path] | /read <file> | /ask <file> <câu hỏi> | /exit");

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let input = line?;
        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "/exit" {
            break;
        }

        if let Some(rest) = trimmed.strip_prefix("/ls") {
            let sub = rest.trim();
            let target_dir = if sub.is_empty() {
                workspace.clone()
            } else {
                resolve_workspace_path(&workspace, sub)?
            };
            let mut tree = format!("{}/\n", target_dir.display());
            list_tree(&workspace, &target_dir, "", 0, 3, &mut tree)?;
            println!("{}", tree);
            continue;
        }

        if let Some(file_path) = trimmed.strip_prefix("/read ") {
            match read_file(&workspace, file_path) {
                Ok(content) => println!("--- {} ---\n{}\n", file_path.trim(), content),
                Err(e) => eprintln!("Lỗi đọc file: {}", e),
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

            let content = read_file(&workspace, file_path)?;
            history.push(Message {
                role: "user".to_string(),
                content: format!(
                    "Dưới đây là nội dung file `{}`:\n```\n{}\n```\n\nCâu hỏi: {}",
                    file_path, content, question
                ),
            });
            print!("DeepSeek: ");
            io::stdout().flush()?;
            send_chat(&client, &mut history)?;
            continue;
        }

        history.push(Message {
            role: "user".to_string(),
            content: trimmed.to_string(),
        });
        print!("DeepSeek: ");
        io::stdout().flush()?;
        send_chat(&client, &mut history)?;
    }

    Ok(())
}
