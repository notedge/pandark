// gate: G-STORE-2
// fixture: store.memory.contract_suite

mod store_contract;

#[test]
fn g_store_memory_contract_suite() {
    store_contract::run_memory_suite();
}
