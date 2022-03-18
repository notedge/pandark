use std::fmt;

use boost_diagnostic::{Diagnostic, DiagnosticCause, DiagnosticCode, DiagnosticOrigin, DiagnosticSeverity, Message};

/// Pandark orchestration error — structured diagnostic with stable code,
/// typed args, cause chain, and origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrawlError {
    /// Structured diagnostic — the only error variant.
    Diagnostic(Diagnostic),
}

impl CrawlError {
    /// Extract the stable diagnostic code.
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Diagnostic(d) => Some(d.code().as_str()),
        }
    }

    /// Extract the diagnostic message key.
    pub fn message_key(&self) -> Option<&str> {
        match self {
            Self::Diagnostic(d) => Some(d.message().key()),
        }
    }
}

impl fmt::Display for CrawlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Diagnostic(d) => {
                if let Some(fallback) = d.message().fallback() {
                    write!(f, "{}", fallback)
                } else {
                    write!(f, "{}/{}", d.origin().namespace(), d.code())
                }
            }
        }
    }
}

impl std::error::Error for CrawlError {}

impl From<Diagnostic> for CrawlError {
    fn from(value: Diagnostic) -> Self {
        Self::Diagnostic(value)
    }
}

/// Build a [`Diagnostic`] with origin, code, severity, message key + args,
/// and optional fallback text. Convenience for production error construction.
pub fn build_diagnostic(
    origin: DiagnosticOrigin,
    code: DiagnosticCode,
    severity: DiagnosticSeverity,
    message: Message,
) -> Diagnostic {
    Diagnostic::new(code, severity, origin, message)
}

/// Pandark result alias.
pub type Result<T> = std::result::Result<T, CrawlError>;
