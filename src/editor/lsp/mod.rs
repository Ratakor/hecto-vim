use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::thread;

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: RequestId,
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    String(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<RequestId>,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: Value,
}

#[derive(Debug)]
pub enum LspMessage {
    Response(JsonRpcResponse),
    Notification(JsonRpcNotification),
}

pub struct LspClient {
    child: Child,
    request_id_counter: i64,
}

impl LspClient {
    pub fn new(server_cmd: &str, args: &[&str]) -> Option<(Self, Receiver<LspMessage>)> {
        let mut child = Command::new(server_cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        let stdout = child.stdout.take()?;
        let (tx, rx) = channel();

        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() || line.is_empty() {
                    break;
                }

                if line.starts_with("Content-Length: ") {
                    let length: usize =
                        line["Content-Length: ".len()..].trim().parse().unwrap_or(0);

                    // Read until the empty line after headers
                    loop {
                        line.clear();
                        if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
                            break;
                        }
                    }

                    let mut body = vec![0; length];
                    if reader.read_exact(&mut body).is_ok()
                        && let Ok(value) = serde_json::from_slice::<Value>(&body)
                    {
                        if value.get("id").is_some() {
                            if let Ok(response) = serde_json::from_value::<JsonRpcResponse>(value) {
                                let _ = tx.send(LspMessage::Response(response));
                            }
                        } else if let Ok(notification) =
                            serde_json::from_value::<JsonRpcNotification>(value)
                        {
                            let _ = tx.send(LspMessage::Notification(notification));
                        }
                    }
                }
            }
        });

        Some((
            Self {
                child,
                request_id_counter: 0,
            },
            rx,
        ))
    }

    pub fn send_request(&mut self, method: &str, params: Value) -> RequestId {
        self.request_id_counter += 1;
        let id = RequestId::Number(self.request_id_counter);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: id.clone(),
            method: method.to_string(),
            params,
        };
        self.send_json(&request);
        id
    }

    pub fn send_notification(&mut self, method: &str, params: Value) {
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        };
        self.send_json(&notification);
    }

    fn send_json<T: Serialize>(&mut self, value: &T) {
        if let Ok(json) = serde_json::to_string(value)
            && let Some(mut stdin) = self.child.stdin.as_ref()
        {
            let payload = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);
            let _ = stdin.write_all(payload.as_bytes());
            let _ = stdin.flush();
        }
    }
}

pub struct LspManager {
    clients: HashMap<crate::editor::FileType, (LspClient, Receiver<LspMessage>)>,
}

impl LspManager {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    pub fn get_client(&mut self, file_type: crate::editor::FileType) -> Option<&mut LspClient> {
        if let std::collections::hash_map::Entry::Vacant(e) = self.clients.entry(file_type) {
            let (cmd, args) = file_type.lsp_server()?;

            if let Some((mut client, rx)) = LspClient::new(cmd, &args) {
                // Initialize
                let params = json!({
                    "processId": std::process::id(),
                    "rootUri": format!("file://{}", std::env::current_dir().unwrap_or_default().display()),
                    "capabilities": {
                        "textDocument": {
                            "publishDiagnostics": {
                                "relatedInformation": true,
                                "versionSupport": false,
                                "tagSupport": {
                                    "valueSet": [1, 2]
                                },
                                "codeDescriptionSupport": true,
                                "dataSupport": true
                            },
                            "hover": {
                                "contentFormat": ["markdown", "plaintext"]
                            },
                            "definition": {
                                "dynamicRegistration": true
                            }
                        }
                    }
                });
                client.send_request("initialize", params);
                client.send_notification("initialized", json!({}));

                e.insert((client, rx));
            } else {
                return None;
            }
        }
        self.clients.get_mut(&file_type).map(|(c, _)| c)
    }

    pub fn poll_messages(&self) -> Vec<(crate::editor::FileType, LspMessage)> {
        let mut messages = Vec::new();
        for (file_type, (_, rx)) in &self.clients {
            while let Ok(msg) = rx.try_recv() {
                messages.push((*file_type, msg));
            }
        }
        messages
    }
}
