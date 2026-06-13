//! Tests du jalon J5 — pricer Monte-Carlo de base.
//!
//! Critère : un call européen prixé par MC colle à Black-Scholes (tol. 1 %).

use kontract::ast::{and, at, give, konst, one, scale, spot, when, Contract};
use kontract::pricer::{price_gbm, McConfig};
use kontract::simulator::Gbm;

// --- Black-Scholes de référence ---------------------------------------------

/// Approximation rationnelle d'erf (Abramowitz-Stegun 7.1.26, ~1e-7).
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

fn bs_call(s: f64, k: f64, r: f64, sigma: f64, t: f64) -> f64 {
    let d1 = ((s / k).ln() + (r + 0.5 * sigma * sigma) * t) / (sigma * t.sqrt());
    let d2 = d1 - sigma * t.sqrt();
    s * norm_cdf(d1) - k * (-r * t).exp() * norm_cdf(d2)
}

fn european_call(asset: &str, k: f64, t: f64) -> Contract {
    when(
        at(t),
        scale((spot(asset) - konst(k)).max(konst(0.0)), one("USD")),
    )
}

// --- Flux déterministes (pas d'aléa) ----------------------------------------

fn det_cfg(rate: f64) -> McConfig {
    McConfig {
        n_paths: 16,
        seed: 1,
        steps_per_year: 4,
        rate,
    }
}

fn flat_model() -> Gbm {
    // sigma = 0 → trajectoire déterministe : S_t = S0 e^{rt}.
    Gbm::new("AAPL", 100.0, 0.05, 0.0)
}

#[test]
fn zero_is_worthless() {
    let res = price_gbm(&Contract::Zero, &flat_model(), &det_cfg(0.05)).unwrap();
    assert_eq!(res.price, 0.0);
}

#[test]
fn one_paid_now_is_par() {
    let res = price_gbm(&one("USD"), &flat_model(), &det_cfg(0.05)).unwrap();
    assert!((res.price - 1.0).abs() < 1e-12);
}

#[test]
fn give_negates() {
    let res = price_gbm(&give(one("USD")), &flat_model(), &det_cfg(0.05)).unwrap();
    assert!((res.price + 1.0).abs() < 1e-12);
}

#[test]
fn and_sums_flows() {
    let c = and(one("USD"), and(one("USD"), one("USD")));
    let res = price_gbm(&c, &flat_model(), &det_cfg(0.0)).unwrap();
    assert!((res.price - 3.0).abs() < 1e-12);
}

#[test]
fn scale_multiplies_by_constant() {
    let c = scale(konst(7.5), one("USD"));
    let res = price_gbm(&c, &flat_model(), &det_cfg(0.0)).unwrap();
    assert!((res.price - 7.5).abs() < 1e-12);
}

#[test]
fn when_discounts_a_zero_coupon_bond() {
    // when(at(1), one) sous taux 5 % → exp(-0.05) ≈ 0.951229.
    let r = 0.05;
    let res = price_gbm(&when(at(1.0), one("USD")), &flat_model(), &det_cfg(r)).unwrap();
    assert!((res.price - (-r * 1.0_f64).exp()).abs() < 1e-12);
}

#[test]
fn scale_samples_observable_at_flow_date() {
    // scale(S, when(at(1), one)) : S échantillonné à t=1, actualisé.
    // Modèle déterministe sigma=0 : S_1 = 100 e^{0.05}. Actualisation e^{-0.05}.
    // Prix attendu = 100 e^{0.05} · e^{-0.05} = 100.
    let c = scale(spot("AAPL"), when(at(1.0), one("USD")));
    let res = price_gbm(&c, &flat_model(), &det_cfg(0.05)).unwrap();
    assert!((res.price - 100.0).abs() < 1e-9, "prix = {}", res.price);
}

#[test]
fn european_call_matches_black_scholes() {
    let (s0, k, r, sigma, t) = (100.0, 100.0, 0.05, 0.20, 1.0);
    // Drift risque-neutre = r.
    let model = Gbm::new("AAPL", s0, r, sigma);
    let cfg = McConfig {
        n_paths: 400_000,
        seed: 2024,
        steps_per_year: 1,
        rate: r,
    };
    let mc = price_gbm(&european_call("AAPL", k, t), &model, &cfg)
        .unwrap()
        .price;
    let bs = bs_call(s0, k, r, sigma, t);

    let rel_err = (mc - bs).abs() / bs;
    assert!(
        rel_err < 0.01,
        "MC = {mc}, BS = {bs}, erreur relative = {rel_err}"
    );
}

#[test]
fn unsupported_combinators_error_before_j6() {
    use kontract::ast::{or, until};
    use kontract::KontractError;

    let res = price_gbm(&or(one("USD"), one("USD")), &flat_model(), &det_cfg(0.0));
    assert!(matches!(res, Err(KontractError::Unsupported(_))));

    let barrier = until(spot("AAPL").ge(konst(200.0)), when(at(1.0), one("USD")));
    let res = price_gbm(&barrier, &flat_model(), &det_cfg(0.0));
    assert!(matches!(res, Err(KontractError::Unsupported(_))));
}
