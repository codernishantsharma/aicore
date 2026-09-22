use aicore_protocol::Response;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatGPTMessageContent {
    pub content_type: Option<String>,
    pub parts: Option<Vec<Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatGPTAuthor {
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatGPTMessage {
    pub id: Option<String>,
    pub author: Option<ChatGPTAuthor>,
    pub content: Option<ChatGPTMessageContent>,
    pub status: Option<String>,
    pub end_turn: Option<bool>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatGPTStreamChunk {
    pub conversation_id: Option<String>,
    pub message: Option<ChatGPTMessage>,
    pub error: Option<Value>,
}

pub struct SseParser {
    last_text: String,
}

impl SseParser {
    pub fn new() -> Self {
        Self {
            last_text: String::new(),
        }
    }

    pub fn parse_line(&mut self, req_id: &str, line: &str) -> Option<Response> {
        let line = line.trim();
        if !line.starts_with("data: ") {
            return None;
        }

        let raw_data = &line[6..].trim();
        if *raw_data == "[DONE]" {
            return Some(Response::done(req_id));
        }

        let parsed: ChatGPTStreamChunk = match serde_json::from_str(raw_data) {
            Ok(c) => c,
            Err(_) => return None,
        };

        if let Some(err) = parsed.error {
            let msg = err
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("ChatGPT streaming error");
            return Some(Response::error(req_id, "PROVIDER_ERROR", msg));
        }

        if let Some(msg) = parsed.message {
            if let Some(role) = msg.author.and_then(|a| a.role) {
                if role == "assistant" {
                    if let Some(content) = msg.content {
                        if let Some(parts) = content.parts {
                            let mut full_text = String::new();
                            for part in parts {
                                if let Some(s) = part.as_str() {
                                    full_text.push_str(s);
                                }
                            }

                            if full_text.len() >= self.last_text.len()
                                && full_text.starts_with(&self.last_text)
                            {
                                let delta = full_text[self.last_text.len()..].to_string();
                                self.last_text = full_text;
                                if !delta.is_empty() {
                                    return Some(Response::delta(
                                        req_id,
                                        serde_json::json!({
                                            "text": delta,
                                            "conversationId": parsed.conversation_id,
                                            "messageId": msg.id
                                        }),
                                    ));
                                }
                            } else {
                                self.last_text = full_text.clone();
                                return Some(Response::delta(
                                    req_id,
                                    serde_json::json!({
                                        "text": full_text,
                                        "conversationId": parsed.conversation_id,
                                        "messageId": msg.id
                                    }),
                                ));
                            }
                        }
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_parser() {
        let mut parser = SseParser::new();

        let line1 = r#"data: {"conversation_id":"conv_1","message":{"id":"msg_1","author":{"role":"assistant"},"content":{"parts":["Hello"]}}}"#;
        let res1 = parser.parse_line("req_1", line1).unwrap();
        if let Response::Delta { data, .. } = res1 {
            assert_eq!(data.get("text").and_then(|v| v.as_str()), Some("Hello"));
        } else {
            panic!("Expected Delta");
        }

        let line2 = r#"data: {"conversation_id":"conv_1","message":{"id":"msg_1","author":{"role":"assistant"},"content":{"parts":["Hello world"]}}}"#;
        let res2 = parser.parse_line("req_1", line2).unwrap();
        if let Response::Delta { data, .. } = res2 {
            assert_eq!(data.get("text").and_then(|v| v.as_str()), Some(" world"));
        } else {
            panic!("Expected Delta");
        }

        let line_done = "data: [DONE]";
        let res_done = parser.parse_line("req_1", line_done).unwrap();
        assert!(matches!(res_done, Response::Done { .. }));
    }
}
