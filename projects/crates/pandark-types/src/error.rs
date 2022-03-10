use std::fmt;

use boost_diagnostic::{Diagnostic, DiagnosticCause, DiagnosticCode, DiagnosticOrigin, DiagnosticSeverity, Message};

/// Pandark orchestration error — primary variant carries a structured
/// [`boost_diagnostic::Diagnostic`] with stable code, typed args, cause chain,
/// and origin. Free-form string variants remain during the migration window;
/// new code SHOULD construct [`Diagnostic`] values directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrawlError {
    /// Structured diagnostic error — the primary production path.
    Diagnostic(Diagnostic),
    /// Invalid user input or configuration (legacy, being migrated to
    /// [`Diagnostic`] with [`crate::diagnostics::REQUEST_INVALID`] /
    /// [`crate::diagnostics::URL_INVALID`] codes).
    InvalidInput(String),
    /// Requested work is not implemented yet (legacy, being migrated to
    /// [`Diagnostic`] with [`crate::diagnostics::EXTRACT_UNSUPPORTED_MEDIA`]
    /// or an equivalent capability code).
    NotImplemented(String),
}

impl CrawlError {
    /// Extract the stable diagnostic code if this error carries one.
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Diagnostic(d) => Some(d.code().as_str()),
            Self::InvalidInput(_) => Some("pandark.request.invalid"),
            Self::NotImplemented(_) => Some("pandark.capability.unimplemented"),
        }
    }

    /// Extract the diagnostic message key if present.
    pub fn message_key(&self) -> Option<&str> {
        match self {
            Self::Diagnostic(d) => Some(d.message().key()),
            Self::InvalidInput(_) | Self::NotImplemented(_) => None,
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
            Self::InvalidInput(message) => write!(f, "invalid input: {message}"),
            Self::NotImplemented(message) => write!(f, "not implemented: {message}"),
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
