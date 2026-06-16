//! Tests du jalon J27 — Basket N-actifs (CorrelatedGbmN).
//!
//! Critères de complétion (ROADMAP.md) :
//!   - basket 3 actifs vs BS(σ_basket) < 3 % ;
//!   - spread option vs formule de Margrabe < 3 % ;
//!   - corrélation empirique ≈ cible (ρ=0.7, tolérance ±0.02) ;
//!   - ρ=1 (corrélation parfaite) → basket (S1+S2)/2 = S1 (< 2 %) ;
//!   - validation du constructeur (erreurs attendues).

use kontract::ast::{at, konst, one, scale, spot, when};
use kontract::numerics::black_scholes_call;
use kontract::pricer::McConfig;
use kontract::{price_gbm, Contract, CorrelatedGbmN, GbmFactor, Simulator};

fn mc(n_paths: usize, rate: f64, seed: u64) -> McConfig {
    McConfig {
        n_paths,
        seed,
        steps_per_year: 50,
        rate,
        variance_reduction: None,
    }
}

/// CorrelatedGbmN à 2 actifs reproduit la corrélation cible des log-rendements.
#[test]
fn test_correlated_gbm_n_recovers_rho() {
    let rho = 0.7;
    let model = CorrelatedGbmN::new(
        vec![
            GbmFactor::new("S1", 100.0, 0.0, 0.25),
            GbmFactor::new("S2", 100.0, 0.0, 0.20),
        ],
        vec![vec![1.0, rho], vec![rho, 1.0]],
    )
    .unwrap();

    let grid = [0.0, 1.0];
    let paths = model.simulate_paths(&grid, 100_000, 42).unwrap();

    let (la, lb): (Vec<f64>, Vec<f64>) = paths
        .iter()
        .map(|p| {
            let s1 = p.spot("S1", 1).unwrap();
            let s2 = p.spot("S2", 1).unwrap();
            ((s1 / 100.0).ln(), (s2 / 100.0).ln())
        })
        .unzip();

    let n = la.len() as f64;
    let ma = la.iter().sum::<f64>() / n;
    let mb = lb.iter().sum::<f64>() / n;
    let cov = la
        .iter()
        .zip(&lb)
        .map(|(a, b)| (a - ma) * (b - mb))
        .sum::<f64>()
        / n;
    let sa = (la.iter().map(|a| (a - ma).powi(2)).sum::<f64>() / n).sqrt();
    let sb = (lb.iter().map(|b| (b - mb).powi(2)).sum::<f64>() / n).sqrt();
    let corr = cov / (sa * sb);

    assert!(
        (corr - rho).abs() < 0.02,
        "corrélation empirique {corr:.4} vs cible {rho:.4}"
    );
}

/// Basket 3 actifs, poids égaux, σ identiques, ρ uniforme :
/// σ_basket = σ · √((1 + 2ρ) / 3) donne une approximation log-normale.
/// Prix MC vs Black-Scholes(σ_basket), tolérance < 3 %.
#[test]
fn test_basket_three_assets_vs_black_scholes() {
    let (s0, k, t, r, sigma, rho) = (100.0, 100.0, 1.0, 0.03, 0.20, 0.5_f64);

    let corr = vec![
        vec![1.0, rho, rho],
        vec![rho, 1.0, rho],
        vec![rho, rho, 1.0],
    ];
    let model = CorrelatedGbmN::new(
        vec![
            GbmFactor::new("S1", s0, r, sigma),
            GbmFactor::new("S2", s0, r, sigma),
            GbmFactor::new("S3", s0, r, sigma),
        ],
        corr,
    )
    .unwrap();

    // Payoff : max((S1+S2+S3)/3 − K, 0) réglé en USD.
    let basket = (spot("S1") + spot("S2") + spot("S3")) / 3.0;
    let contract: Contract = when(at(t), scale((basket - konst(k)).clip(0.0), one("USD")));

    let res = price_gbm(&contract, &model, &mc(200_000, r, 7)).unwrap();

    // Volatilité effective du basket (approx. log-normale, σ égaux, ρ uniforme).
    let sigma_basket = sigma * ((1.0 + 2.0 * rho) / 3.0).sqrt();
    let analytic = black_scholes_call(s0, k, t, r, sigma_basket);

    let rel = (res.price - analytic).abs() / analytic;
    assert!(
        rel < 0.03,
        "basket MC {:.5} vs BS(σ_basket={sigma_basket:.4}) {analytic:.5} (rel {rel:.4})",
        res.price
    );
}

/// Spread option (S1 − S2)⁺ vs formule de Margrabe :
/// `C = BS(S1₀, S2₀, T, 0, σ_spread)` avec `σ²_spread = σ1² + σ2² − 2ρσ1σ2`.
#[test]
fn test_spread_option_vs_margrabe() {
    let (s0, t, r, sigma1, sigma2, rho) = (100.0, 1.0, 0.03, 0.20, 0.20, 0.5_f64);

    let model = CorrelatedGbmN::new(
        vec![
            GbmFactor::new("S1", s0, r, sigma1),
            GbmFactor::new("S2", s0, r, sigma2),
        ],
        vec![vec![1.0, rho], vec![rho, 1.0]],
    )
    .unwrap();

    // Payoff : max(S1 − S2, 0) à la maturité.
    let spread = (spot("S1") - spot("S2")).clip(0.0);
    let contract: Contract = when(at(t), scale(spread, one("USD")));

    let res = price_gbm(&contract, &model, &mc(300_000, r, 11)).unwrap();

    // Margrabe ≡ BS(S1₀, S2₀, T, r=0, σ_spread) (exchange option, r s'annule).
    let sigma_spread = (sigma1 * sigma1 + sigma2 * sigma2 - 2.0 * rho * sigma1 * sigma2).sqrt();
    let analytic = black_scholes_call(s0, s0, t, 0.0, sigma_spread);

    let rel = (res.price - analytic).abs() / analytic;
    assert!(
        rel < 0.03,
        "spread MC {:.5} vs Margrabe {analytic:.5} (σ_spread={sigma_spread:.4}, rel {rel:.4})",
        res.price
    );
}

/// ρ=1 (corrélation parfaite, mêmes paramètres) : basket (S1+S2)/2 = S1 sur chaque path.
/// Prix basket call ≈ BS(s0, K, T, r, σ) à moins de 2 %.
#[test]
fn test_perfect_correlation_degenerates_to_single_asset() {
    let (s0, k, t, r, sigma) = (100.0, 100.0, 1.0, 0.03, 0.20);

    let model = CorrelatedGbmN::new(
        vec![
            GbmFactor::new("S1", s0, r, sigma),
            GbmFactor::new("S2", s0, r, sigma),
        ],
        vec![vec![1.0, 1.0], vec![1.0, 1.0]],
    )
    .unwrap();

    let basket = (spot("S1") + spot("S2")) / 2.0;
    let contract: Contract = when(at(t), scale((basket - konst(k)).clip(0.0), one("USD")));

    let res = price_gbm(&contract, &model, &mc(100_000, r, 3)).unwrap();
    let analytic = black_scholes_call(s0, k, t, r, sigma);

    let rel = (res.price - analytic).abs() / analytic;
    assert!(
        rel < 0.02,
        "basket ρ=1 MC {:.5} vs BS {analytic:.5} (rel {rel:.4})",
        res.price
    );
}

/// Validation du constructeur : moins de 2 actifs ou matrice de mauvaise taille → erreur.
#[test]
fn test_constructor_validates_inputs() {
    let single = CorrelatedGbmN::new(
        vec![GbmFactor::new("S1", 100.0, 0.03, 0.20)],
        vec![vec![1.0]],
    );
    assert!(single.is_err(), "un seul actif doit retourner une erreur");

    let wrong_size = CorrelatedGbmN::new(
        vec![
            GbmFactor::new("S1", 100.0, 0.03, 0.20),
            GbmFactor::new("S2", 100.0, 0.03, 0.20),
            GbmFactor::new("S3", 100.0, 0.03, 0.20),
        ],
        vec![vec![1.0, 0.5], vec![0.5, 1.0]],
    );
    assert!(
        wrong_size.is_err(),
        "matrice 2×2 pour 3 actifs doit retourner une erreur"
    );
}
