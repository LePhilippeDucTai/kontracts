//! Tests du jalon J2 — évaluation des observables sur paths synthétiques.

use kontract::ast::{konst, spot};
use kontract::observable::Path;
use kontract::KontractError;

/// Path à deux sous-jacents sur 3 dates.
fn sample_path() -> Path {
    Path::new(vec![0.0, 0.5, 1.0])
        .with_asset("AAPL", vec![100.0, 110.0, 150.0])
        .and_then(|p| p.with_asset("MSFT", vec![200.0, 190.0, 210.0]))
        .expect("path cohérent")
}

#[test]
fn const_is_time_invariant() {
    let p = sample_path();
    for t in 0..p.len() {
        assert_eq!(konst(42.0).eval(&p, t).unwrap(), 42.0);
    }
}

#[test]
fn spot_reads_the_trajectory() {
    let p = sample_path();
    assert_eq!(spot("AAPL").eval(&p, 0).unwrap(), 100.0);
    assert_eq!(spot("AAPL").eval(&p, 2).unwrap(), 150.0);
    assert_eq!(spot("MSFT").eval(&p, 1).unwrap(), 190.0);
}

#[test]
fn arithmetic_combines_spots() {
    let p = sample_path();
    // (AAPL + MSFT) / 2 au pas 2 = (150 + 210) / 2 = 180
    let mid = (spot("AAPL") + spot("MSFT")) / konst(2.0);
    assert_eq!(mid.eval(&p, 2).unwrap(), 180.0);

    // AAPL * 2 - 50 au pas 1 = 110*2 - 50 = 170
    let expr = spot("AAPL") * konst(2.0) - konst(50.0);
    assert_eq!(expr.eval(&p, 1).unwrap(), 170.0);

    // -AAPL au pas 0 = -100
    assert_eq!((-spot("AAPL")).eval(&p, 0).unwrap(), -100.0);
}

#[test]
fn max_min_implement_call_put_payoffs() {
    let p = sample_path();
    // Payoff call max(S - K, 0), K = 120, à maturité (pas 2) : max(150-120,0)=30
    let call = (spot("AAPL") - konst(120.0)).max(konst(0.0));
    assert_eq!(call.eval(&p, 2).unwrap(), 30.0);
    // Au pas 0 le call est hors de la monnaie : max(100-120,0)=0
    assert_eq!(call.eval(&p, 0).unwrap(), 0.0);

    // Payoff put max(K - S, 0), K = 120, au pas 0 : max(120-100,0)=20
    let put = (konst(120.0) - spot("AAPL")).max(konst(0.0));
    assert_eq!(put.eval(&p, 0).unwrap(), 20.0);

    // min plafonne : min(AAPL, 130) au pas 2 = min(150,130) = 130
    let capped = spot("AAPL").min(konst(130.0));
    assert_eq!(capped.eval(&p, 2).unwrap(), 130.0);
}

#[test]
fn nested_expression() {
    let p = sample_path();
    // max( (AAPL - MSFT) , 0 ) au pas 2 = max(150-210,0) = 0
    let spread = (spot("AAPL") - spot("MSFT")).max(konst(0.0));
    assert_eq!(spread.eval(&p, 2).unwrap(), 0.0);
}

#[test]
fn unknown_asset_errors() {
    let p = sample_path();
    let err = spot("TSLA").eval(&p, 0).unwrap_err();
    assert!(matches!(err, KontractError::UnknownAsset(name) if name == "TSLA"));
}

#[test]
fn step_out_of_range_errors() {
    let p = sample_path();
    let err = spot("AAPL").eval(&p, 99).unwrap_err();
    assert!(matches!(err, KontractError::StepOutOfRange(99)));
}

#[test]
fn inconsistent_path_is_rejected() {
    let res = Path::new(vec![0.0, 1.0]).with_asset("AAPL", vec![100.0]); // 1 valeur, 2 dates
    assert!(matches!(res, Err(KontractError::InconsistentPath(_))));
}
