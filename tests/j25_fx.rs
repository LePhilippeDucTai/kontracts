//! Tests du jalon J25 — FX simple (multi-devise, corrélation spot/FX).
//!
//! Critère : « Cross-currency options ». On valide :
//!   - option de change vanille via le moteur existant vs Garman-Kohlhagen ;
//!   - quanto (drift ajusté par ρ·σ_S·σ_X) vs analytique + monotonie en ρ ;
//!   - composite `S·X` via simulation **deux GBM corrélés** vs Black-Scholes
//!     sur le produit log-normal (volatilité combinée).

use kontract::ast::{at, konst, one, scale, spot, when};
use kontract::fx::{
    composite_vol, fx_forward, garman_kohlhagen_call, garman_kohlhagen_put, quanto_call,
    CorrelatedGbm2, GbmFactor,
};
use kontract::numerics::black_scholes_call;
use kontract::pricer::McConfig;
use kontract::products::european_call;
use kontract::{price_gbm, Contract, Gbm};

fn mc(n_paths: usize, rate: f64, seed: u64) -> McConfig {
    McConfig {
        n_paths,
        seed,
        steps_per_year: 50,
        rate,
        variance_reduction: None,
    }
}

/// Parité put-call de Garman-Kohlhagen : `C − P = X₀e^{−r_f t} − Ke^{−r_d t}`.
#[test]
fn test_garman_kohlhagen_put_call_parity() {
    let (x0, k, t, r_d, r_f, sigma) = (1.20, 1.25, 1.0, 0.04, 0.01, 0.10);
    let c = garman_kohlhagen_call(x0, k, t, r_d, r_f, sigma);
    let p = garman_kohlhagen_put(x0, k, t, r_d, r_f, sigma);
    let parity = x0 * (-r_f * t).exp() - k * (-r_d * t).exp();
    assert!(
        (c - p - parity).abs() < 1e-10,
        "GK parité : C−P {:.8} vs {:.8}",
        c - p,
        parity
    );
}

/// **Option de change vanille via le moteur** : un call sur le taux `X`, price
/// sous un `Gbm` de drift `r_d − r_f` actualisé à `r_d`, retrouve le prix
/// Garman-Kohlhagen analytique — sans aucun code moteur spécifique au FX.
#[test]
fn test_fx_vanilla_engine_vs_garman_kohlhagen() {
    let (x0, k, t, r_d, r_f, sigma) = (1.10, 1.10, 1.0, 0.03, 0.01, 0.12);

    // L'actif "EURUSD" suit un GBM de drift r_d − r_f sous la mesure domestique.
    let model = Gbm::new("EURUSD", x0, r_d - r_f, sigma);
    let contract = european_call("EURUSD", k, t, "USD");
    let res = price_gbm(&contract, &model, &mc(200_000, r_d, 7)).expect("price");

    let analytic = garman_kohlhagen_call(x0, k, t, r_d, r_f, sigma);
    assert!(
        (res.price - analytic).abs() < 5e-3,
        "FX call moteur {:.5} vs GK {:.5} (CI [{:.5}, {:.5}])",
        res.price,
        analytic,
        res.ci95_low,
        res.ci95_high
    );
}

/// Forward FX = parité des taux couverte.
#[test]
fn test_fx_forward_interest_rate_parity() {
    let f = fx_forward(1.20, 2.0, 0.04, 0.01);
    assert!((f - 1.20 * (0.06_f64).exp()).abs() < 1e-12);
}

/// **Quanto call via le moteur** : actif étranger `S` à drift ajusté
/// `r_f − ρσ_Sσ_X`, actualisé en domestique `r_d`, retrouve l'analytique quanto.
#[test]
fn test_quanto_call_engine_vs_analytic() {
    let (s0, k, t) = (100.0, 100.0, 1.0);
    let (r_d, r_f, q_s) = (0.04, 0.02, 0.0);
    let (sigma_s, sigma_x, rho) = (0.20, 0.10, 0.30);

    let mu_q = r_f - q_s - rho * sigma_s * sigma_x;
    let model = Gbm::new("STOCK", s0, mu_q, sigma_s);
    let contract = european_call("STOCK", k, t, "USD");
    let res = price_gbm(&contract, &model, &mc(300_000, r_d, 13)).expect("price");

    let analytic = quanto_call(s0, k, t, r_d, r_f, q_s, sigma_s, sigma_x, rho);
    assert!(
        (res.price - analytic).abs() < 5e-3,
        "quanto call moteur {:.5} vs analytique {:.5} (CI [{:.5}, {:.5}])",
        res.price,
        analytic,
        res.ci95_low,
        res.ci95_high
    );
}

/// La corrélation `ρ` déplace le drift quanto (`−ρσ_Sσ_X`) : pour un call, la
/// valeur **décroît** quand `ρ` augmente.
#[test]
fn test_quanto_monotonic_in_correlation() {
    let (s0, k, t, r_d, r_f, q_s, sig_s, sig_x) = (100.0, 100.0, 1.0, 0.04, 0.02, 0.0, 0.25, 0.15);
    let neg = quanto_call(s0, k, t, r_d, r_f, q_s, sig_s, sig_x, -0.5);
    let zero = quanto_call(s0, k, t, r_d, r_f, q_s, sig_s, sig_x, 0.0);
    let pos = quanto_call(s0, k, t, r_d, r_f, q_s, sig_s, sig_x, 0.5);
    assert!(
        neg > zero && zero > pos,
        "quanto call doit décroître en ρ : {neg:.5} > {zero:.5} > {pos:.5}"
    );
}

/// **Composite (cross-currency) via simulation deux GBM corrélés** : le payoff
/// `max(S_T·X_T − K, 0)` réglé en domestique price comme un Black-Scholes sur
/// `U = S·X` (log-normal de volatilité combinée), drift global `r_d`.
#[test]
fn test_composite_option_correlated_two_factor() {
    let (s0, x0) = (100.0, 1.20);
    let u0 = s0 * x0;
    let (sigma_s, sigma_x, rho) = (0.20, 0.12, 0.40);
    let t = 1.0;
    let r_d = 0.03;
    let r_f = 0.01;
    let k = 130.0;

    // Drifts choisis pour que le produit U = S·X croisse au taux domestique :
    // drift_X = r_d − r_f, drift_S = r_f − ρσ_Sσ_X  ⇒  d_S + d_X + ρσ_Sσ_X = r_d.
    let drift_x = r_d - r_f;
    let drift_s = r_f - rho * sigma_s * sigma_x;
    let model = CorrelatedGbm2::new(
        GbmFactor::new("STOCK", s0, drift_s, sigma_s),
        GbmFactor::new("FX", x0, drift_x, sigma_x),
        rho,
    );

    // Payoff DSL : max(S·X − K, 0) payé en USD à t.
    let payoff = scale(
        (spot("STOCK") * spot("FX") - konst(k)).max(konst(0.0)),
        one("USD"),
    );
    let contract: Contract = when(at(t), payoff);

    let res = price_gbm(&contract, &model, &mc(400_000, r_d, 23)).expect("price");

    let sigma_u = composite_vol(sigma_s, sigma_x, rho);
    let analytic = black_scholes_call(u0, k, t, r_d, sigma_u);
    let rel = (res.price - analytic).abs() / analytic;
    assert!(
        rel < 0.02,
        "composite MC {:.5} vs BS(U) {:.5} (rel {:.4}, CI [{:.5}, {:.5}])",
        res.price,
        analytic,
        rel,
        res.ci95_low,
        res.ci95_high
    );
}

/// Le simulateur corrélé reproduit la corrélation cible des log-rendements.
#[test]
fn test_correlated_gbm_recovers_rho() {
    use kontract::Simulator;
    let rho = 0.6;
    let model = CorrelatedGbm2::new(
        GbmFactor::new("A", 100.0, 0.0, 0.30),
        GbmFactor::new("B", 50.0, 0.0, 0.20),
        rho,
    );
    let grid = [0.0, 1.0];
    let paths = model.simulate_paths(&grid, 100_000, 5).expect("sim");

    // Log-rendements terminaux des deux facteurs.
    let (la, lb): (Vec<f64>, Vec<f64>) = paths
        .iter()
        .map(|p| {
            let a = p.spot("A", 1).unwrap();
            let b = p.spot("B", 1).unwrap();
            ((a / 100.0).ln(), (b / 50.0).ln())
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
