use std::fmt;

/// Pandark orchestration error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrawlError {
    /// Invalid user input or configuration.
    InvalidInput(String),
    /// Requested work is not implemented yet.
    NotImplemented(String),
}

impl fmt::Display for CrawlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(f, "invalid input: {message}"),
            Self::NotImplemented(message) => write!(f, "not implemented: {message}"),
        }
    }
}

impl std::error::Error for CrawlError {}

/// Pandark result alias.
pub type Result<T> = std::result::Result<T, CrawlError>;
