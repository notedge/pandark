use std::fmt;

/// Stable store-level errors exposed to Pandark core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// Lease token expired or was reclaimed.
    StoreLeaseExpired,
    /// Claim token does not match the active frontier claim.
    StoreFencingMismatch,
    /// Compare-and-set or state conflict.
    StoreConflict,
    /// Namespace quota exceeded.
    StoreQuotaExceeded,
    /// Stored schema requires migration.
    StoreMigrationRequired,
    /// Unrecoverable inconsistency.
    StoreCorrupt(String),
    /// Run or resource not found.
    StoreNotFound(String),
    /// Invalid operation for the current store state.
    StoreInvalidState(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StoreLeaseExpired => write!(f, "StoreLeaseExpired"),
            Self::StoreFencingMismatch => write!(f, "StoreFencingMismatch"),
            Self::StoreConflict => write!(f, "StoreConflict"),
            Self::StoreQuotaExceeded => write!(f, "StoreQuotaExceeded"),
            Self::StoreMigrationRequired => write!(f, "StoreMigrationRequired"),
            Self::StoreCorrupt(message) => write!(f, "StoreCorrupt: {message}"),
            Self::StoreNotFound(message) => write!(f, "StoreNotFound: {message}"),
            Self::StoreInvalidState(message) => write!(f, "StoreInvalidState: {message}"),
        }
    }
}

impl std::error::Error for StoreError {}

/// Convenient result alias for store operations.
pub type Result<T> = std::result::Result<T, StoreError>;
