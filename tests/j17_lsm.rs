//! Jalon J17 — Américaines par Longstaff-Schwartz (LSM).
//!
//! Validation contre :
//!   - les prix d'arbres binomiaux Cox-Ross-Rubinstein (référence américaine),
//!   - les prix Black-Scholes (européens, bornes de cohérence).

use kontract::ast::{konst, one, scale, spot};
use kontract::{price_american_lsm, price_gbm, Contract, Gbm, LsmConfig, McConfig};

// ============================================================================
// Références analytiques / arbres
// ============================================================================

/// N(x) via erfc (Abramowitz-Stegun 7.1.26).
fn norm_cdf(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x_abs = x.abs() / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.327_591_1 * x_abs);
    let poly = ((((1.061_405_429 * t - 1.453_152_027) * t) + 1.421_413_741) * t - 0.284_496_736)
        * t
        + 0.254_829_592;
    let erf_abs = 1.0 - poly * t * (-x_abs * x_abs).exp();
    0.5 * (1.0 + sign * erf_abs)
}

fn bs_call(s: f64, k: f64, t: f64, r: f64, sigma: f64) -> f64 {
    let d1 = ((s / k).ln() + (r + 0.5 * sigma * sigma) * t) / (sigma * t.sqrt());
    let d2 = d1 - sigma * t.sqrt();
    s * norm_cdf(d1) - k * (-r * t).exp() * norm_cdf(d2)
}

fn bs_put(s: f64, k: f64, t: f64, r: f64, sigma: f64) -> f64 {
    let d1 = ((s / k).ln() + (r + 0.5 * sigma * sigma) * t) / (sigma * t.sqrt());
    let d2 = d1 - sigma * t.sqrt();
    k * (-r * t).exp() * norm_cdf(-d2) - s * norm_cdf(-d1)
}

/// Arbre binomial Cox-Ross-Rubinstein pour une option américaine.
/// `is_call` sélectionne le payoff `max(S − K, 0)` (call) ou `max(K − S, 0)` (put).
fn crr_american(s0: f64, k: f64, t: f64, r: f64, sigma: f64, n: usize, is_call: bool) -> f64 {
    let dt = t / n as f64;
    let u = (sigma * dt.sqrt()).exp();
    let d = 1.0 / u;
    let disc = (-r * dt).exp();
    let p = ((r * dt).exp() - d) / (u - d);

    let payoff = |s: f64| -> f64 {
        if is_call {
            (s - k).max(0.0)
        } else {
            (k - s).max(0.0)
        }
    };

    // Valeurs à maturité.
    let mut values: Vec<f64> = (0..=n)
        .map(|j| {
            let s = s0 * u.powi(j as i32) * d.powi((n - j) as i32);
            payoff(s)
        })
        .collect();

    // Backward induction avec exercice anticipé.
    for step in (0..n).rev() {
        for j in 0..=step {
            let s = s0 * u.powi(j as i32) * d.powi((step - j) as i32);
            let cont = disc * (p * values[j + 1] + (1.0 - p) * values[j]);
            values[j] = cont.max(payoff(s));
        }
    }
    values[0]
}

// ============================================================================
// Helpers de construction de contrats / dates d'exercice
// ============================================================================

/// Payoff exercé d'un put américain : `max(K − S, 0)` payé en `ccy`.
fn american_put_payoff(asset: &str, k: f64, ccy: &str) -> Contract {
    scale((konst(k) - spot(asset)).max(konst(0.0)), one(ccy))
}

/// Payoff exercé d'un call américain : `max(S − K, 0)` payé en `ccy`.
fn american_call_payoff(asset: &str, k: f64, ccy: &str) -> Contract {
    scale((spot(asset) - konst(k)).max(konst(0.0)), one(ccy))
}

/// Dates d'exercice : `n` dates uniformes sur `(0, t]`.
fn exercise_dates(t: f64, n: usize) -> Vec<f64> {
    (1..=n).map(|i| t * i as f64 / n as f64).collect()
}

fn cfg(rate: f64) -> McConfig {
    McConfig {
        n_paths: 60_000,
        seed: 7,
        steps_per_year: 50,
        rate,
        variance_reduction: None,
    }
}

// ============================================================================
// Tests
// ============================================================================

/// Test 1 — Un call américain (sans dividende) ne doit jamais être exercé tôt :
/// son prix doit coïncider avec le call européen Black-Scholes.
#[test]
fn american_call_equals_european_call() {
    let (s0, k, t, r, sigma) = (100.0, 95.0, 1.0, 0.05, 0.2);
    let gbm = Gbm::new("AAPL", s0, r, sigma);
    let payoff = american_call_payoff("AAPL", k, "USD");
    let dates = exercise_dates(t, 20);

    let lsm = price_american_lsm(&payoff, &dates, &gbm, &cfg(r), &LsmConfig::default())
        .expect("LSM call");
    let bs = bs_call(s0, k, t, r, sigma);

    // L'exercice anticipé d'un call sans dividende n'apporte rien → ≈ européen.
    let rel = (lsm.price - bs).abs() / bs;
    assert!(
        rel < 0.02,
        "call US {:.4} vs BS européen {:.4} (rel {:.4})",
        lsm.price,
        bs,
        rel
    );
}

/// Test 2 — Un put américain ITM vaut **strictement plus** que le put européen
/// correspondant (prime d'exercice anticipé).
#[test]
fn american_put_exceeds_european_put() {
    let (s0, k, t, r, sigma) = (100.0, 110.0, 1.0, 0.06, 0.25);
    let gbm = Gbm::new("AAPL", s0, r, sigma);
    let payoff = american_put_payoff("AAPL", k, "USD");
    let dates = exercise_dates(t, 20);

    let lsm =
        price_american_lsm(&payoff, &dates, &gbm, &cfg(r), &LsmConfig::default()).expect("LSM put");
    let bs = bs_put(s0, k, t, r, sigma);

    assert!(
        lsm.price > bs,
        "put US {:.4} devrait dépasser put EU {:.4}",
        lsm.price,
        bs
    );
    // Au moins une prime mesurable (et pas absurde).
    assert!(lsm.price - bs > 0.05, "prime d'exercice trop faible");
}

/// Test 3 — Call américain ATM vs arbre CRR : accord à ~1 %.
#[test]
fn american_call_matches_crr() {
    let (s0, k, t, r, sigma) = (100.0, 100.0, 1.0, 0.05, 0.2);
    let gbm = Gbm::new("AAPL", s0, r, sigma);
    let payoff = american_call_payoff("AAPL", k, "USD");
    let dates = exercise_dates(t, 50);

    let lsm = price_american_lsm(&payoff, &dates, &gbm, &cfg(r), &LsmConfig::default())
        .expect("LSM call");
    let crr = crr_american(s0, k, t, r, sigma, 500, true);

    let rel = (lsm.price - crr).abs() / crr;
    assert!(
        rel < 0.01,
        "call US LSM {:.4} vs CRR {:.4} (rel {:.4})",
        lsm.price,
        crr,
        rel
    );
}

/// Test 4 — Put américain ITM vs arbre CRR : accord à ~1 %.
#[test]
fn american_put_matches_crr() {
    let (s0, k, t, r, sigma) = (100.0, 100.0, 1.0, 0.06, 0.25);
    let gbm = Gbm::new("AAPL", s0, r, sigma);
    let payoff = american_put_payoff("AAPL", k, "USD");
    let dates = exercise_dates(t, 50);

    let lsm =
        price_american_lsm(&payoff, &dates, &gbm, &cfg(r), &LsmConfig::default()).expect("LSM put");
    let crr = crr_american(s0, k, t, r, sigma, 500, false);

    let rel = (lsm.price - crr).abs() / crr;
    assert!(
        rel < 0.01,
        "put US LSM {:.4} vs CRR {:.4} (rel {:.4})",
        lsm.price,
        crr,
        rel
    );
    // Et reste sous le prix d'un exercice immédiat majoré : cohérence.
    assert!(lsm.price >= (k - s0).max(0.0));
}

/// Test 5 — Sensibilité à la base : n_basis = 2 et n_basis = 3 convergent vers le
/// même prix CRR (la base linéaire suffit déjà, la quadratique la confirme).
#[test]
fn basis_sensitivity_converges() {
    let (s0, k, t, r, sigma) = (100.0, 105.0, 1.0, 0.05, 0.3);
    let gbm = Gbm::new("AAPL", s0, r, sigma);
    let payoff = american_put_payoff("AAPL", k, "USD");
    let dates = exercise_dates(t, 40);

    let p2 = price_american_lsm(&payoff, &dates, &gbm, &cfg(r), &LsmConfig { n_basis: 2 })
        .expect("n_basis=2")
        .price;
    let p3 = price_american_lsm(&payoff, &dates, &gbm, &cfg(r), &LsmConfig { n_basis: 3 })
        .expect("n_basis=3")
        .price;
    let crr = crr_american(s0, k, t, r, sigma, 500, false);

    // Les deux bases s'accordent entre elles…
    assert!(
        (p2 - p3).abs() / p3 < 0.02,
        "n_basis=2 ({:.4}) vs n_basis=3 ({:.4}) divergent",
        p2,
        p3
    );
    // …et avec la référence CRR.
    assert!(
        (p3 - crr).abs() / crr < 0.02,
        "p3 {:.4} vs CRR {:.4}",
        p3,
        crr
    );
}

/// Sanity — l'API standard `price_gbm` reste intacte (pas de régression).
#[test]
fn standard_pricer_unaffected() {
    let call = kontract::products::european_call("AAPL", 100.0, 1.0, "USD");
    let gbm = Gbm::new("AAPL", 100.0, 0.05, 0.2);
    let res = price_gbm(&call, &gbm, &cfg(0.05)).expect("EU call");
    let bs = bs_call(100.0, 100.0, 1.0, 0.05, 0.2);
    assert!((res.price - bs).abs() / bs < 0.02);
}
