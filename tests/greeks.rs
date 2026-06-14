//! Tests du jalon J7 — Greeks par bump-and-reprice (CRN) vs Black-Scholes.

use kontract::ast::{at, konst, one, scale, spot, when, Contract};
use kontract::greeks::{greeks_gbm, BumpSizes};
use kontract::pricer::McConfig;

// --- Greeks analytiques Black-Scholes ---------------------------------------

fn erf(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let y = 1.0
        - (((((1.061_405_429 * t - 1.453_152_027) * t) + 1.421_413_741) * t - 0.284_496_736) * t
            + 0.254_829_592)
            * t
            * (-x * x).exp();
    sign * y
}

fn norm_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

fn norm_pdf(x: f64) -> f64 {
    (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt()
}

struct BsGreeks {
    delta: f64,
    gamma: f64,
    vega: f64,
    rho: f64,
}

fn bs_call_greeks(s: f64, k: f64, r: f64, sigma: f64, t: f64) -> BsGreeks {
    let st = sigma * t.sqrt();
    let d1 = ((s / k).ln() + (r + 0.5 * sigma * sigma) * t) / st;
    let d2 = d1 - st;
    BsGreeks {
        delta: norm_cdf(d1),
        gamma: norm_pdf(d1) / (s * st),
        vega: s * norm_pdf(d1) * t.sqrt(),
        rho: k * t * (-r * t).exp() * norm_cdf(d2),
    }
}

fn european_call(asset: &str, k: f64, t: f64) -> Contract {
    when(
        at(t),
        scale((spot(asset) - konst(k)).max(konst(0.0)), one("USD")),
    )
}

fn cfg() -> McConfig {
    McConfig {
        n_paths: 500_000,
        seed: 2024,
        steps_per_year: 1,
        rate: 0.05,
        variance_reduction: None,
    }
}

#[test]
fn call_delta_matches_black_scholes() {
    let g = greeks_gbm(
        &european_call("AAPL", 100.0, 1.0),
        "AAPL",
        100.0,
        0.20,
        &cfg(),
        &BumpSizes::default(),
    )
    .unwrap();
    let bs = bs_call_greeks(100.0, 100.0, 0.05, 0.20, 1.0);
    assert!(
        (g.delta - bs.delta).abs() / bs.delta < 0.01,
        "delta MC = {}, BS = {}",
        g.delta,
        bs.delta
    );
}

#[test]
fn call_vega_matches_black_scholes() {
    let g = greeks_gbm(
        &european_call("AAPL", 100.0, 1.0),
        "AAPL",
        100.0,
        0.20,
        &cfg(),
        &BumpSizes::default(),
    )
    .unwrap();
    let bs = bs_call_greeks(100.0, 100.0, 0.05, 0.20, 1.0);
    assert!(
        (g.vega - bs.vega).abs() / bs.vega < 0.02,
        "vega MC = {}, BS = {}",
        g.vega,
        bs.vega
    );
}

#[test]
fn call_rho_matches_black_scholes() {
    let g = greeks_gbm(
        &european_call("AAPL", 100.0, 1.0),
        "AAPL",
        100.0,
        0.20,
        &cfg(),
        &BumpSizes::default(),
    )
    .unwrap();
    let bs = bs_call_greeks(100.0, 100.0, 0.05, 0.20, 1.0);
    assert!(
        (g.rho - bs.rho).abs() / bs.rho < 0.02,
        "rho MC = {}, BS = {}",
        g.rho,
        bs.rho
    );
}

#[test]
fn call_gamma_matches_black_scholes() {
    // Gamma = différence seconde : CRN indispensable. Bump spot plus large.
    let bumps = BumpSizes {
        spot: 1.0,
        ..BumpSizes::default()
    };
    let g = greeks_gbm(
        &european_call("AAPL", 100.0, 1.0),
        "AAPL",
        100.0,
        0.20,
        &cfg(),
        &bumps,
    )
    .unwrap();
    let bs = bs_call_greeks(100.0, 100.0, 0.05, 0.20, 1.0);
    assert!(
        (g.gamma - bs.gamma).abs() / bs.gamma < 0.05,
        "gamma MC = {}, BS = {}",
        g.gamma,
        bs.gamma
    );
}

#[test]
fn delta_is_bounded_for_a_call() {
    // Le delta d'un call est dans [0, 1].
    let g = greeks_gbm(
        &european_call("AAPL", 100.0, 1.0),
        "AAPL",
        100.0,
        0.20,
        &cfg(),
        &BumpSizes::default(),
    )
    .unwrap();
    assert!(g.delta > 0.0 && g.delta < 1.0);
    assert!(g.gamma > 0.0); // convexité positive
    assert!(g.vega > 0.0);
}
