// gate: G-STORE-4
// fixture: store.doctor.quota_orphan

#[cfg(feature = "yydb")]
mod store_contract;

#[cfg(feature = "yydb")]
#[test]
fn g_store_doctor_quota_orphan() {
    store_contract::run_yydb_doctor_suite();
}
