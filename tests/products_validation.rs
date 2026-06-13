//! Tests du jalon J9 — suite de validation des produits vs formules fermées.
//!
//! Critère : tous les prix dans les tolérances. Chaque produit est une simple
//! expression du DSL (`kontract::products`) — le moteur n'en connaît aucun.

use kontract::pricer::{price_gbm, McConfig};
use kontract::products::{
    bull_call_spread, cash_or_nothing_call, down_and_out_call, european_call, european_put,
    forward, straddle, up_and_out_call, zero_coupon_bond,
};
use kontract::simulator::Gbm;

const S0: f64 = 100.0;
const R: f64 = 0.05;
const SIGMA: f64 = 0.20;
const T: f64 = 1.0;

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
fn n(x: f64) -> f64 {
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}
fn d1(s: f64, k: f64) -> f64 {
    ((s / k).ln() + (R + 0.5 * SIGMA * SIGMA) * T) / (SIGMA * T.sqrt())
}
fn bs_call(s: f64, k: f64) -> f64 {
    let d1 = d1(s, k);
    let d2 = d1 - SIGMA * T.sqrt();
    s * n(d1) - k * (-R * T).exp() * n(d2)
}
fn bs_put(s: f64, k: f64) -> f64 {
    bs_call(s, k) - s + k * (-R * T).exp() // parité call-put
}

fn model() -> Gbm {
    Gbm::new("AAPL", S0, R, SIGMA)
}
fn cfg(steps: usize) -> McConfig {
    McConfig {
        n_paths: 400_000,
        seed: 2024,
        steps_per_year: steps,
        rate: R,
    }
}

fn price(c: &kontract::ast::Contract, steps: usize) -> f64 {
    price_gbm(c, &model(), &cfg(steps)).unwrap().price
}

#[test]
fn vanilla_call_and_put() {
    let c = price(&european_call("AAPL", 100.0, T, "USD"), 1);
    assert!((c - bs_call(S0, 100.0)).abs() / bs_call(S0, 100.0) < 0.01);

    let p = price(&european_put("AAPL", 100.0, T, "USD"), 1);
    assert!((p - bs_put(S0, 100.0)).abs() / bs_put(S0, 100.0) < 0.01);
}

#[test]
fn zero_coupon_bond_discounts() {
    let zc = price(&zero_coupon_bond("USD", T), 1);
    assert!((zc - (-R * T).exp()).abs() < 1e-9);
}

#[test]
fn forward_value() {
    // Forward de strike K : valeur = S0 − K e^{−rT}.
    // On choisit K assez bas pour que la valeur soit grande devant le bruit MC
    // (le payoff S_T − K a un écart-type ~20, indépendant de sa moyenne).
    let k = 90.0;
    let f = price(&forward("AAPL", k, T, "USD"), 1);
    let theo = S0 - k * (-R * T).exp();
    assert!((f - theo).abs() / theo.abs() < 0.01, "f={f}, theo={theo}");
}

#[test]
fn straddle_equals_call_plus_put() {
    let s = price(&straddle("AAPL", 100.0, T, "USD"), 1);
    let theo = bs_call(S0, 100.0) + bs_put(S0, 100.0);
    assert!((s - theo).abs() / theo < 0.01);
}

#[test]
fn bull_spread_equals_call_difference() {
    let s = price(&bull_call_spread("AAPL", 95.0, 110.0, T, "USD"), 1);
    let theo = bs_call(S0, 95.0) - bs_call(S0, 110.0);
    assert!((s - theo).abs() / theo < 0.02, "spread={s}, theo={theo}");
}

#[test]
fn digital_cash_or_nothing_call() {
    // Cash-or-nothing call : payout · e^{−rT} · N(d2).
    let payout = 10.0;
    let d = price(&cash_or_nothing_call("AAPL", 100.0, payout, T, "USD"), 1);
    let d2 = d1(S0, 100.0) - SIGMA * T.sqrt();
    let theo = payout * (-R * T).exp() * n(d2);
    assert!((d - theo).abs() / theo < 0.02, "digital={d}, theo={theo}");
}

#[test]
fn down_and_out_call_below_vanilla() {
    let ko = price(&down_and_out_call("AAPL", 100.0, 90.0, T, "USD"), 250);
    let vanilla = bs_call(S0, 100.0);
    assert!(ko > 0.0 && ko < vanilla, "ko={ko}, vanilla={vanilla}");
}

#[test]
fn up_and_out_call_below_vanilla() {
    let ko = price(&up_and_out_call("AAPL", 100.0, 130.0, T, "USD"), 250);
    let vanilla = bs_call(S0, 100.0);
    assert!(ko > 0.0 && ko < vanilla, "ko={ko}, vanilla={vanilla}");
}

#[test]
fn knock_out_recovers_vanilla_when_barrier_far() {
    // Barrière très haute → up-and-out ≈ vanille.
    let ko = price(&up_and_out_call("AAPL", 100.0, 10_000.0, T, "USD"), 50);
    let vanilla = bs_call(S0, 100.0);
    assert!((ko - vanilla).abs() / vanilla < 0.01);
}
