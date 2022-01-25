// gate: G-STORE-4
// fixture: store.migration.v1_to_v2

mod store_contract;

#[test]
fn g_store_migration_v1_to_v2_memory() {
    store_contract::run_migration_suite("memory");
}

#[cfg(feature = "yydb")]
#[test]
fn g_store_migration_v1_to_v2_yydb() {
    store_contract::run_migration_suite("yydb");
}
