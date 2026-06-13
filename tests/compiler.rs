//! Tests du jalon J4 — compilateur AST → plan de calcul.
//!
//! Critère : plan correct sur 5 contrats de référence.

use kontract::ast::{and, anytime, at, konst, one, scale, spot, until, when, Contract};
use kontract::compiler::compile;
use kontract::KontractError;

/// Call européen `max(S - K, 0)` payé en USD à `T`.
fn european_call(asset: &str, k: f64, t: f64) -> Contract {
    when(
        at(t),
        scale((spot(asset) - konst(k)).max(konst(0.0)), one("USD")),
    )
}

// --- Les 5 contrats de référence --------------------------------------------

#[test]
fn ref1_zero_coupon_bond() {
    // when(at(1), one) : pas d'actif, une date, pas de barrière.
    let plan = compile(&when(at(1.0), one("USD"))).unwrap();
    assert!(plan.assets.is_empty());
    assert_eq!(plan.fixed_dates, vec![1.0]);
    assert_eq!(plan.horizon, 1.0);
    assert!(!plan.needs_fine_grid);
    // Grille européenne : juste {0, 1}.
    assert_eq!(plan.time_grid(50), vec![0.0, 1.0]);
}

#[test]
fn ref2_european_call() {
    let plan = compile(&european_call("AAPL", 150.0, 1.0)).unwrap();
    assert_eq!(plan.assets, vec!["AAPL".to_string()]);
    assert_eq!(plan.fixed_dates, vec![1.0]);
    assert_eq!(plan.horizon, 1.0);
    assert!(!plan.needs_fine_grid);
}

#[test]
fn ref3_knock_out_call_needs_fine_grid() {
    // Barrière prix : until(S >= 200, call) → monitoring fin requis.
    let ko = until(
        spot("AAPL").ge(konst(200.0)),
        european_call("AAPL", 150.0, 1.0),
    );
    let plan = compile(&ko).unwrap();
    assert_eq!(plan.assets, vec!["AAPL".to_string()]);
    assert_eq!(plan.fixed_dates, vec![1.0]);
    assert_eq!(plan.horizon, 1.0);
    assert!(plan.needs_fine_grid);

    // Grille dense : 0, 1/12, …, 1 → 13 points pour 12 pas/an.
    let grid = plan.time_grid(12);
    assert_eq!(grid.len(), 13);
    assert_eq!(*grid.first().unwrap(), 0.0);
    assert_eq!(*grid.last().unwrap(), 1.0);
}

#[test]
fn ref4_basket_two_assets_sorted() {
    // Panier AAPL + MSFT : deux actifs, triés déterministe.
    let basket = when(at(1.0), scale(spot("MSFT") + spot("AAPL"), one("USD")));
    let plan = compile(&basket).unwrap();
    assert_eq!(plan.assets, vec!["AAPL".to_string(), "MSFT".to_string()]);
    assert!(!plan.needs_fine_grid);
}

#[test]
fn ref5_composite_multiple_dates_with_barrier() {
    // and(coupon@0.5, KO call@1.0) : deux dates + barrière.
    let coupon = when(at(0.5), one("USD"));
    let ko = until(
        spot("AAPL").ge(konst(150.0)),
        european_call("AAPL", 120.0, 1.0),
    );
    let plan = compile(&and(coupon, ko)).unwrap();
    assert_eq!(plan.assets, vec!["AAPL".to_string()]);
    assert_eq!(plan.fixed_dates, vec![0.5, 1.0]);
    assert_eq!(plan.horizon, 1.0);
    assert!(plan.needs_fine_grid);
    // La grille dense inclut la date intermédiaire 0.5.
    assert!(plan.time_grid(10).iter().any(|t| (t - 0.5).abs() < 1e-12));
}

// --- Cas additionnels -------------------------------------------------------

#[test]
fn zero_has_empty_plan() {
    let plan = compile(&Contract::Zero).unwrap();
    assert!(plan.assets.is_empty());
    assert!(plan.fixed_dates.is_empty());
    assert_eq!(plan.horizon, 0.0);
    assert!(!plan.needs_fine_grid);
    assert_eq!(plan.time_grid(12), vec![0.0]);
}

#[test]
fn anytime_with_price_condition_is_a_barrier() {
    // American-style : exerçable tant que S <= 80.
    let amer = anytime(spot("AAPL").le(konst(80.0)), when(at(2.0), one("USD")));
    let plan = compile(&amer).unwrap();
    assert!(plan.needs_fine_grid);
    assert_eq!(plan.horizon, 2.0);
}

#[test]
fn negative_date_is_malformed() {
    let bad = when(at(-1.0), one("USD"));
    assert!(matches!(
        compile(&bad),
        Err(KontractError::MalformedContract(_))
    ));
}

#[test]
fn duplicate_dates_are_merged() {
    // Deux acquisitions à la même date ne produisent qu'une entrée.
    let c = and(when(at(1.0), one("USD")), when(at(1.0), one("EUR")));
    let plan = compile(&c).unwrap();
    assert_eq!(plan.fixed_dates, vec![1.0]);
}
