use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

// Error code constants
pub const ERR_AUTH_REQUIRED: &str = "AUTH_REQUIRED";
pub const ERR_AUTH_EXPIRED: &str = "AUTH_EXPIRED";
pub const ERR_AUTH_FAILED: &str = "AUTH_FAILED";
pub const ERR_PROVIDER_UNAVAILABLE: &str = "PROVIDER_UNAVAILABLE";
pub const ERR_PROVIDER_ERROR: &str = "PROVIDER_ERROR";
pub const ERR_INVALID_REQUEST: &str = "INVALID_REQUEST";
pub const ERR_INVALID_SESSION: &str = "INVALID_SESSION";
pub const ERR_INVALID_FILE: &str = "INVALID_FILE";
pub const ERR_UPLOAD_FAILED: &str = "UPLOAD_FAILED";
pub const ERR_DOWNLOAD_FAILED: &str = "DOWNLOAD_FAILED";
pub const ERR_STREAM_ERROR: &str = "STREAM_ERROR";
pub const ERR_TIMEOUT: &str = "TIMEOUT";
pub const ERR_PERMISSION_DENIED: &str = "PERMISSION_DENIED";
pub const ERR_NOT_FOUND: &str = "NOT_FOUND";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub id: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub version: u32,
    pub id: String,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default)]
    pub client: Option<ClientInfo>,
}

impl Request {
    pub fn new(id: impl Into<String>, method: impl Into<String>, params: serde_json::Value) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            id: id.into(),
            method: method.into(),
            params,
            client: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Response {
    Result {
        version: u32,
        id: String,
        result: serde_json::Value,
    },
    Delta {
        version: u32,
        id: String,
        data: serde_json::Value,
    },
    Done {
        version: u32,
        id: String,
    },
    Error {
        version: u32,
        id: String,
        error: ProtocolError,
    },
}

impl Response {
    pub fn result(id: impl Into<String>, result: serde_json::Value) -> Self {
        Self::Result {
            version: PROTOCOL_VERSION,
            id: id.into(),
            result,
        }
    }

    pub fn delta(id: impl Into<String>, data: serde_json::Value) -> Self {
        Self::Delta {
            version: PROTOCOL_VERSION,
            id: id.into(),
            data,
        }
    }

    pub fn done(id: impl Into<String>) -> Self {
        Self::Done {
            version: PROTOCOL_VERSION,
            id: id.into(),
        }
    }

    pub fn error(id: impl Into<String>, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Error {
            version: PROTOCOL_VERSION,
            id: id.into(),
            error: ProtocolError::new(code, message),
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Result { id, .. } => id,
            Self::Delta { id, .. } => id,
            Self::Done { id, .. } => id,
            Self::Error { id, .. } => id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProtocolError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ProtocolError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let req = Request::new("req_123", "chat.send", serde_json::json!({
            "sessionId": "session_1",
            "message": "Hello world"
        }));
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"version\":1"));
        assert!(json.contains("\"id\":\"req_123\""));
        assert!(json.contains("\"method\":\"chat.send\""));

        let deserialized: Request = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, req);
    }

    #[test]
    fn test_response_variants() {
        let res_result = Response::result("req_123", serde_json::json!({"text": "Hi"}));
        let json_res = serde_json::to_string(&res_result).unwrap();
        assert!(json_res.contains("\"type\":\"result\""));

        let res_delta = Response::delta("req_123", serde_json::json!({"text": "H"}));
        let json_delta = serde_json::to_string(&res_delta).unwrap();
        assert!(json_delta.contains("\"type\":\"delta\""));

        let res_done = Response::done("req_123");
        let json_done = serde_json::to_string(&res_done).unwrap();
        assert!(json_done.contains("\"type\":\"done\""));

        let res_err = Response::error("req_123", ERR_AUTH_REQUIRED, "Authentication required");
        let json_err = serde_json::to_string(&res_err).unwrap();
        assert!(json_err.contains("\"type\":\"error\""));
        assert!(json_err.contains("\"code\":\"AUTH_REQUIRED\""));
    }
}
