use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::time::Duration;

const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434/api/chat";
pub const DEFAULT_MODEL: &str = "qwen2.5-coder:7b";
pub const MAX_TOOL_STEPS: usize = 5;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ToolFunctionCall {
    pub name: String,
    pub arguments: Value,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ToolCall {
    pub function: ToolFunctionCall,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Message {
    pub role: String,
    #[serde(default)]
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Serialize)]
struct FunctionSchema {
    name: &'static str,
    description: &'static str,
    parameters: Value,
}

#[derive(Serialize)]
struct ToolSpec {
    #[serde(rename = "type")]
    kind: &'static str,
    function: FunctionSchema,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [ToolSpec]>,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: Message,
}

pub fn workspace_tools() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            kind: "function",
            function: FunctionSchema {
                name: "list_tree",
                description: "Liệt kê cây thư mục bên trong workspace.",
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Đường dẫn thư mục tương đối, mặc định là '.'" },
                        "max_depth": { "type": "integer", "description": "Độ sâu tối đa, mặc định là 3" }
                    }
                }),
            },
        },
        ToolSpec {
            kind: "function",
            function: FunctionSchema {
                name: "read_file",
                description: "Đọc nội dung một file trong workspace.",
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Đường dẫn file tương đối trong workspace" }
                    },
                    "required": ["path"]
                }),
            },
        },
        ToolSpec {
            kind: "function",
            function: FunctionSchema {
                name: "search_workspace",
                description: "Tìm từ khóa (không phân biệt hoa thường) trong các file mã nguồn của workspace.",
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Từ khóa cần tìm" },
                        "max_hits": { "type": "integer", "description": "Số dòng kết quả tối đa, mặc định là 20" }
                    },
                    "required": ["query"]
                }),
            },
        },
    ]
}

pub struct OllamaClient {
    http: Client,
    url: String,
    pub model: String,
}

impl OllamaClient {
    pub fn new(model: &str) -> Result<Self, Box<dyn Error>> {
        let http = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(300))
            .build()?;

        Ok(Self {
            http,
            url: DEFAULT_OLLAMA_URL.to_string(),
            model: model.to_string(),
        })
    }

    pub fn set_model(&mut self, new_model: &str) {
        self.model = new_model.trim().to_string();
    }

    fn post_once(
        &self,
        history: &[Message],
        tools: Option<&[ToolSpec]>,
    ) -> Result<Message, Box<dyn Error>> {
        let request = ChatRequest {
            model: &self.model,
            messages: history,
            stream: false,
            tools,
        };

        let resp: ChatResponse = self
            .http
            .post(&self.url)
            .json(&request)
            .send()?
            .error_for_status()?
            .json()?;

        Ok(resp.message)
    }

    pub fn send_with_tools<F>(
        &self,
        history: &mut Vec<Message>,
        mut execute_tool: F,
    ) -> Result<String, Box<dyn Error>>
    where
        F: FnMut(&str, &Value) -> String,
    {
        let tools = workspace_tools();

        for _step in 0..MAX_TOOL_STEPS {
            let msg = self.post_once(history, Some(&tools))?;

            if let Some(calls) = &msg.tool_calls {
                if !calls.is_empty() {
                    history.push(msg.clone());
                    for call in calls {
                        let name = &call.function.name;
                        println!("[Tool -> {}] {}", name, call.function.arguments);
                        let output = execute_tool(name, &call.function.arguments);
                        history.push(Message {
                            role: "tool".to_string(),
                            content: output,
                            tool_calls: None,
                        });
                    }
                    continue;
                }
            }

            let reply = msg.content.clone();
            println!("{}", reply);
            history.push(Message {
                role: "assistant".to_string(),
                content: reply.clone(),
                tool_calls: None,
            });
            return Ok(reply);
        }

        Err("Đã đạt giới hạn số bước gọi tool tối đa".into())
    }
}
