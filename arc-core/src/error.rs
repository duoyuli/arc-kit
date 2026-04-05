use std::fmt::{Display, Formatter};

#[derive(Debug, Clone)]
pub struct ArcError {
    pub message: String,
    pub hint: Option<String>,
    /// Optional override for the process exit code (default is 1).
    pub exit_code: Option<u8>,
}

impl ArcError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            hint: None,
            exit_code: None,
        }
    }

    pub fn with_hint(message: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            hint: Some(hint.into()),
            exit_code: None,
        }
    }

    pub fn with_exit_code(mut self, code: u8) -> Self {
        self.exit_code = Some(code);
        self
    }
}

impl Display for ArcError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(hint) = &self.hint {
            write!(f, "{} ({hint})", self.message)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

impl std::error::Error for ArcError {}

pub type Result<T> = std::result::Result<T, ArcError>;
