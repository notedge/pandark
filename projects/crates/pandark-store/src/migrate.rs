//! Store schema migration helpers.

/// Current `store/schema_version` written by `pandark-store`.
pub const CURRENT_STORE_SCHEMA_VERSION: u32 = 2;

/// Legacy pre-envelope schema marker used by contract fixtures.
pub const LEGACY_STORE_SCHEMA_VERSION: u32 = 0;

/// Apply inline migrations for the active store session.
pub fn migrate_store_schema(stored: u32) -> u32 {
    match stored {
        LEGACY_STORE_SCHEMA_VERSION => CURRENT_STORE_SCHEMA_VERSION,
        version => version,
    }
}
