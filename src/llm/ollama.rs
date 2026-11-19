use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::{BufRead, BufReader};
use std::time::Duration;

const OLLAMA_URL: &str = "http://localhost:11434/api/chat";
const MODEL: &str = "deepseek-coder:6.7b";

#[derive(Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
}

#[derive(Deserialize)]
struct StreamResponse {
    message: Option<Message>,
    done: bool,
}

pub fn build_client() -> Result<Client, Box<dyn Error>> {
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(300))
        .build()?;
    Ok(client)
}

pub fn send_chat(client: &Client, history: &mut Vec<Message>) -> Result<String, Box<dyn Error>> {
    let request = ChatRequest {
        model: MODEL.to_string(),
        messages: history.clone(),
        stream: true,
    };

    let response = client
        .post(OLLAMA_URL)
        .json(&request)
        .send()?
        .error_for_status()?;

    let reader = BufReader::new(response);
    let mut reply = String::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let chunk: StreamResponse = serde_json::from_str(&line)?;
        if let Some(msg) = chunk.message {
            print!("{}", msg.content);
            reply.push_str(&msg.content);
        }
        if chunk.done {
            break;
        }
    }

    println!();
    history.push(Message {
        role: "assistant".to_string(),
        content: reply.clone(),
    });

    Ok(reply)
}
