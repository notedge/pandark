// gate: G-STORE-3
// fixture: store.yydb.parity

#[cfg(feature = "yydb")]
mod store_contract;

#[cfg(feature = "yydb")]
#[test]
fn g_store_yydb_parity() {
    store_contract::run_yydb_suite();
}
