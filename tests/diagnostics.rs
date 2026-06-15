//! Tests du jalon J5b — diagnostics Monte-Carlo (erreur standard, IC 95 %).

use kontract::ast::{at, konst, one, scale, spot, when, Contract};
use kontract::pricer::{price_gbm, McConfig};
use kontract::simulator::Gbm;

fn european_call(asset: &str, k: f64, t: f64) -> Contract {
    when(
        at(t),
        scale((spot(asset) - konst(k)).max(konst(0.0)), one("USD")),
    )
}

#[test]
fn deterministic_payoff_has_zero_error() {
    // sigma = 0 → toutes les trajectoires identiques → erreur nulle.
    let model = Gbm::new("AAPL", 100.0, 0.05, 0.0);
    let cfg = McConfig {
        n_paths: 1000,
        seed: 1,
        steps_per_year: 1,
        rate: 0.05,
        variance_reduction: None,
    };
    let res = price_gbm(&when(at(1.0), one("USD")), &model, &cfg).unwrap();
    assert!(res.sample_std < 1e-12);
    assert!(res.std_error < 1e-12);
    assert!((res.ci95_low - res.price).abs() < 1e-12);
    assert!((res.ci95_high - res.price).abs() < 1e-12);
    assert_eq!(res.n_paths, 1000);
}

#[test]
fn stochastic_call_has_positive_error_and_bracketing_ci() {
    let model = Gbm::new("AAPL", 100.0, 0.05, 0.20);
    let cfg = McConfig {
        n_paths: 100_000,
        seed: 7,
        steps_per_year: 1,
        rate: 0.05,
        variance_reduction: None,
    };
    let res = price_gbm(&european_call("AAPL", 100.0, 1.0), &model, &cfg).unwrap();

    assert!(res.std_error > 0.0);
    assert!(res.ci95_low < res.price && res.price < res.ci95_high);
    // Largeur d'IC cohérente avec 2·1.96·SE.
    let width = res.ci95_high - res.ci95_low;
    assert!((width - 2.0 * 1.959_963_984_540_054 * res.std_error).abs() < 1e-9);
}

#[test]
fn ci_contains_black_scholes_price() {
    // L'IC à 95 % doit (statistiquement) contenir le prix BS ≈ 10.4506.
    let model = Gbm::new("AAPL", 100.0, 0.05, 0.20);
    let cfg = McConfig {
        n_paths: 500_000,
        seed: 2024,
        steps_per_year: 1,
        rate: 0.05,
        variance_reduction: None,
    };
    let res = price_gbm(&european_call("AAPL", 100.0, 1.0), &model, &cfg).unwrap();
    let bs = 10.450_583_572_185_565;
    assert!(
        res.ci95_low <= bs && bs <= res.ci95_high,
        "BS {bs} hors IC [{}, {}]",
        res.ci95_low,
        res.ci95_high
    );
}

#[test]
fn paths_for_tolerance_scales_quadratically() {
    let model = Gbm::new("AAPL", 100.0, 0.05, 0.20);
    let cfg = McConfig {
        n_paths: 50_000,
        seed: 11,
        steps_per_year: 1,
        rate: 0.05,
        variance_reduction: None,
    };
    let res = price_gbm(&european_call("AAPL", 100.0, 1.0), &model, &cfg).unwrap();

    let n_coarse = res.paths_for_tolerance(0.10);
    let n_fine = res.paths_for_tolerance(0.05);
    assert!(n_fine > n_coarse);
    // Diviser la tolérance par 2 → ~4× plus de trajectoires.
    let ratio = n_fine as f64 / n_coarse as f64;
    assert!((ratio - 4.0).abs() < 0.05, "ratio = {ratio}");
}

#[test]
fn paths_for_tolerance_is_zero_for_deterministic() {
    let model = Gbm::new("AAPL", 100.0, 0.05, 0.0);
    let cfg = McConfig {
        n_paths: 100,
        seed: 1,
        steps_per_year: 1,
        rate: 0.0,
        variance_reduction: None,
    };
    let res = price_gbm(&Contract::Zero, &model, &cfg).unwrap();
    assert_eq!(res.paths_for_tolerance(0.01), 0);
}
