//! Verify public exports using an external integration-test crate.
use mathematica_mcp::{FinanceQuery, Mathematica, build_financial_data_code};

#[test]
fn external_crate_can_access_public_library_surface() {
    let _new: fn() -> anyhow::Result<Mathematica> = Mathematica::new;
    let query = FinanceQuery::new("AAPL");
    assert_eq!(query.symbol, "AAPL");
    assert_eq!(build_financial_data_code("AAPL", None, None, None, None).unwrap(), "FinancialData[\"AAPL\"]");
}
