//! Tests du jalon J6 — barrières (`until`, `anytime`).
//!
//! Critère : un call à barrière (knock-out) prixé par MC colle à la formule
//! analytique (continue, corrigée pour le monitoring discret) à 2 %.

use kontract::ast::{anytime, at, konst, one, scale, spot, until, when, Contract};
use kontract::pricer::{price_gbm, McConfig};
use kontract::simulator::Gbm;

// --- Black-Scholes & formule de barrière (Reiner-Rubinstein) ----------------

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

/// Down-and-in call (barrière `h <= k`, `h < s`).
fn down_and_in_call(s: f64, k: f64, h: f64, r: f64, sigma: f64, t: f64) -> f64 {
    let lambda = (r + 0.5 * sigma * sigma) / (sigma * sigma);
    let st = sigma * t.sqrt();
    let y = (h * h / (s * k)).ln() / st + lambda * st;
    s * (h / s).powf(2.0 * lambda) * norm_cdf(y)
        - k * (-r * t).exp() * (h / s).powf(2.0 * lambda - 2.0) * norm_cdf(y - st)
}

/// Down-and-out call = vanille − down-and-in.
fn down_and_out_call(s: f64, k: f64, h: f64, r: f64, sigma: f64, t: f64) -> f64 {
    bs_call(s, k, r, sigma, t) - down_and_in_call(s, k, h, r, sigma, t)
}

fn european_call(asset: &str, k: f64, t: f64) -> Contract {
    when(
        at(t),
        scale((spot(asset) - konst(k)).max(konst(0.0)), one("USD")),
    )
}

// --- Tests ------------------------------------------------------------------

#[test]
fn knock_out_matches_analytic_with_continuity_correction() {
    let (s0, k, h, r, sigma, t) = (100.0, 100.0, 90.0, 0.05, 0.20, 1.0);
    let steps = 250usize;

    // Down-and-out call : KO si S touche/descend sous H.
    let ko = until(spot("AAPL").le(konst(h)), european_call("AAPL", k, t));
    let model = Gbm::new("AAPL", s0, r, sigma);
    let cfg = McConfig {
        n_paths: 300_000,
        seed: 2024,
        steps_per_year: steps,
        rate: r,
    };
    let mc = price_gbm(&ko, &model, &cfg).unwrap().price;

    // Correction de Broadie-Glasserman-Kou pour le monitoring discret.
    let dt = t / steps as f64;
    let h_corr = h * (-0.5826 * sigma * dt.sqrt()).exp();
    let analytic = down_and_out_call(s0, k, h_corr, r, sigma, t);

    let rel = (mc - analytic).abs() / analytic;
    assert!(
        rel < 0.02,
        "MC = {mc}, analytique = {analytic}, rel = {rel}"
    );
}

#[test]
fn unreachable_barrier_recovers_vanilla() {
    // Barrière inatteignable (S <= 1) → le KO vaut la vanille.
    let (s0, k, r, sigma, t) = (100.0, 100.0, 0.05, 0.20, 1.0);
    let ko = until(spot("AAPL").le(konst(1.0)), european_call("AAPL", k, t));
    let model = Gbm::new("AAPL", s0, r, sigma);
    let cfg = McConfig {
        n_paths: 200_000,
        seed: 7,
        steps_per_year: 50,
        rate: r,
    };
    let mc = price_gbm(&ko, &model, &cfg).unwrap().price;
    let bs = bs_call(s0, k, r, sigma, t);
    assert!((mc - bs).abs() / bs < 0.01, "MC = {mc}, BS = {bs}");
}

#[test]
fn immediately_breached_barrier_is_worthless() {
    // S0 = 100 et barrière S <= 200 vraie dès t=0 → knock-out immédiat → 0.
    let ko = until(
        spot("AAPL").le(konst(200.0)),
        european_call("AAPL", 100.0, 1.0),
    );
    let model = Gbm::new("AAPL", 100.0, 0.05, 0.20);
    let cfg = McConfig {
        n_paths: 1000,
        seed: 1,
        steps_per_year: 10,
        rate: 0.05,
    };
    assert_eq!(price_gbm(&ko, &model, &cfg).unwrap().price, 0.0);
}

#[test]
fn anytime_first_touch_pays_when_barrier_reached() {
    // Modèle déterministe (sigma = 0) : S_t = 100 e^{0.05 t}, croissant.
    // anytime(S >= 105, when(at(2), one)) : la barrière 105 est franchie avant
    // t = 2 (à t = ln(1.05)/0.05 ≈ 0.98), donc on acquiert le ZC → e^{-0.05·2}.
    let amer = anytime(spot("AAPL").ge(konst(105.0)), when(at(2.0), one("USD")));
    let model = Gbm::new("AAPL", 100.0, 0.05, 0.0);
    let cfg = McConfig {
        n_paths: 8,
        seed: 1,
        steps_per_year: 12,
        rate: 0.05,
    };
    let price = price_gbm(&amer, &model, &cfg).unwrap().price;
    assert!((price - (-0.1f64).exp()).abs() < 1e-9, "price = {price}");
}

#[test]
fn anytime_never_activated_is_worthless() {
    // Barrière jamais atteinte (S_2 = 100 e^{0.1} ≈ 110.5 < 200) → rien.
    let amer = anytime(spot("AAPL").ge(konst(200.0)), when(at(2.0), one("USD")));
    let model = Gbm::new("AAPL", 100.0, 0.05, 0.0);
    let cfg = McConfig {
        n_paths: 8,
        seed: 1,
        steps_per_year: 12,
        rate: 0.05,
    };
    assert_eq!(price_gbm(&amer, &model, &cfg).unwrap().price, 0.0);
}

#[test]
fn knock_out_plus_knock_in_via_complement() {
    // KO (S<=H) + part « knockée » = vanille : on vérifie que le KO est
    // strictement inférieur à la vanille quand la barrière mord.
    let (s0, k, h, r, sigma, t) = (100.0, 100.0, 95.0, 0.05, 0.20, 1.0);
    let model = Gbm::new("AAPL", s0, r, sigma);
    let cfg = McConfig {
        n_paths: 100_000,
        seed: 3,
        steps_per_year: 100,
        rate: r,
    };
    let ko = until(spot("AAPL").le(konst(h)), european_call("AAPL", k, t));
    let mc_ko = price_gbm(&ko, &model, &cfg).unwrap().price;
    let bs = bs_call(s0, k, r, sigma, t);
    assert!(mc_ko > 0.0 && mc_ko < bs, "KO = {mc_ko}, vanille = {bs}");
}
