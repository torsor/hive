use hive_protocol::ErrorBody;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("transport: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("{code}: {message}")]
    Http {
        status: u16,
        code: String,
        message: String,
    },
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

impl Error {
    pub fn from_response(status: reqwest::StatusCode, text: &str) -> Self {
        if let Ok(err) = serde_json::from_str::<ErrorBody>(text) {
            return Self::Http {
                status: status.as_u16(),
                code: err.code,
                message: err.error,
            };
        }
        Self::Http {
            status: status.as_u16(),
            code: "http".into(),
            message: text.to_string(),
        }
    }
}
