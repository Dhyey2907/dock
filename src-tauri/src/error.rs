use serde::{ser::SerializeStruct, Serialize, Serializer};

/// Application-wide error type. Serialized to the frontend as `{ kind, message }`; the UI maps
/// `kind` to calm copy and never shows `message` directly.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("tauri error: {0}")]
    Tauri(#[from] tauri::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("platform error: {0}")]
    Platform(String),
}

impl AppError {
    pub fn kind(&self) -> &'static str {
        match self {
            AppError::Io(_) => "io",
            AppError::Tauri(_) => "tauri",
            AppError::Json(_) => "json",
            AppError::InvalidInput(_) => "invalid_input",
            AppError::Platform(_) => "platform",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(windows)]
impl From<windows::core::Error> for AppError {
    fn from(error: windows::core::Error) -> Self {
        AppError::Platform(error.to_string())
    }
}
