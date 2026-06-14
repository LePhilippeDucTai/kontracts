//! Tests du jalon J13 — SABR + Merton (Jump-Diffusion).
//!
//! Critères de complétion (6 tests, 3 SABR + 3 Merton) :
//!   SABR-1 : ATM call stable (non-NaN, positif, dans les bornes)
//!   SABR-2 : β=1, ν=0 → limite GBM, call dans les 1 % de Black-Scholes
//!   SABR-3 : ρ impact (OTM calls diffèrent selon le signe de ρ, magnitude et signe corrects)
//!   MERTON-1 : λ=0 → limite GBM/BS, call dans les 1 % de Black-Scholes
//!   MERTON-2 : vs formule fermée Merton (somme BS pondérée Poisson), tolérance 3 %
//!   MERTON-3 : sauts positifs → OTM call plus cher que GBM (impact de queue)

use kontract::ast::{at, konst, one, scale, spot, when, Contract};
use kontract::pricer::{price_gbm, price_on_paths, McConfig};
use kontract::simulator::{merton_from_params, sabr_from_params, Simulator};

// ============================================================================
// Utilitaires communs
// ============================================================================

/// Approximation rationnelle d'erf (Abramowitz & Stegun 7.1.26, erreur ≤ 1.5e-7).
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

/// Formule de Black-Scholes pour un call européen.
fn bs_call(s: f64, k: f64, r: f64, sigma: f64, t: f64) -> f64 {
    if sigma <= 0.0 || t <= 0.0 {
        return (s - k * (-r * t).exp()).max(0.0);
    }
    let d1 = ((s / k).ln() + (r + 0.5 * sigma * sigma) * t) / (sigma * t.sqrt());
    let d2 = d1 - sigma * t.sqrt();
    s * norm_cdf(d1) - k * (-r * t).exp() * norm_cdf(d2)
}

/// Call européen exprimé dans le DSL.
fn european_call(asset: &str, k: f64, t: f64) -> Contract {
    when(
        at(t),
        scale((spot(asset) - konst(k)).max(konst(0.0)), one("USD")),
    )
}

/// Formule fermée de Merton (somme des prix BS pondérée par loi de Poisson).
///
/// C_Merton = Σ_{n=0}^N_max e^{-λT} (λT)^n / n! · C_BS(S·e^{n·μ_j}, K, r_n, σ_n, T)
///
/// où σ_n² = σ² + n·σ_j²/T et r_n = r − λ·κ + n·μ_j/T
/// avec κ = e^{μ_j} − 1 (ajustement risque-neutre).
///
/// Note : on utilise la formulation standard de Merton (1976) :
///   le numérateur S est ajusté par e^{n·μ_j} pour le saut moyen.
fn merton_closed_form(
    s: f64,
    k: f64,
    r: f64,
    sigma: f64,
    t: f64,
    lambda: f64,
    mu_j: f64,
    sigma_j: f64,
) -> f64 {
    let n_max = 20; // termes suffisants pour convergence (Poisson bien tronquée)
    let kappa = mu_j.exp() - 1.0; // E[J] - 1

    // Intensité risque-neutre ajustée
    let lambda_star = lambda * (1.0 + kappa);

    let mut price = 0.0;
    let mut poisson_weight = (-lambda_star * t).exp(); // e^{-λ*T} pour n=0
    let lambda_t = lambda_star * t;

    for n in 0..=n_max {
        if n > 0 {
            poisson_weight *= lambda_t / n as f64;
        }
        // Vol ajustée : σ_n² = σ² + n·σ_j²/T
        let sigma_n_sq = sigma * sigma + (n as f64) * sigma_j * sigma_j / t;
        let sigma_n = sigma_n_sq.sqrt();

        // Drift ajusté : r_n = r − λ·κ + n·ln(1+κ)/T  (Merton 1976)
        let r_n = r - lambda * kappa + (n as f64) * (1.0 + kappa).ln() / t;

        price += poisson_weight * bs_call(s, k, r_n, sigma_n, t);

        // Arrêt si le poids est négligeable
        if poisson_weight < 1e-12 && n > 5 {
            break;
        }
    }
    price
}

/// Config MC pour les tests J13 (nombre de paths suffisant pour < 1-3 %).
fn mc_cfg(r: f64) -> McConfig {
    McConfig {
        n_paths: 400_000,
        seed: 42,
        steps_per_year: 100,
        rate: r,
    }
}

// ============================================================================
// Tests SABR (3 tests)
// ============================================================================

/// SABR-1 : ATM call stable — non-NaN, positif, dans les bornes.
///
/// Avec des paramètres SABR standards (α=0.3, β=0.7, ν=0.4, ρ=-0.3),
/// le prix d'un call ATM doit être :
///   - non-NaN et non-infini
///   - strictement positif (option a de la valeur)
///   - inférieur à S₀ (borne triviale)
///   - dans la fourchette de vol implicite [5 %, 80 %] (sanity check)
#[test]
fn sabr_atm_call_is_stable() {
    let (s0, k, r, t) = (100.0, 100.0, 0.05, 1.0);
    let sabr = sabr_from_params(
        "X", s0, /* alpha */ 0.3, /* beta */ 0.7, /* nu */ 0.4, /* rho */ -0.3,
        r,
    );

    let contract = european_call("X", k, t);
    let cfg = mc_cfg(r);

    let result = price_gbm(&contract, &sabr, &cfg).expect("SABR pricing failed");
    let price = result.price;

    // Bornes de sanity : le prix doit exister et être raisonnable
    assert!(
        price.is_finite(),
        "SABR ATM call price is not finite: {price}"
    );
    assert!(
        price > 0.0,
        "SABR ATM call price is zero or negative: {price}"
    );
    assert!(
        price < s0,
        "SABR ATM call price exceeds S0: {price} vs S0={s0}"
    );

    // Fourchette de vol implicite grossière [5 %, 80 %]
    let bs_low = bs_call(s0, k, r, 0.05, t);
    let bs_high = bs_call(s0, k, r, 0.80, t);
    assert!(
        price >= bs_low && price <= bs_high,
        "SABR ATM call {price:.4} hors fourchette BS [{bs_low:.4}, {bs_high:.4}]"
    );
}

/// SABR-2 : β=1, ν=0 → limite GBM.
///
/// Quand β=1 (mode GBM) et ν=0 (pas de stochasticité de la vol), SABR se réduit
/// à un GBM avec σ = α·S^(β-1) = α (car β-1=0). Le prix du call doit être dans
/// les 1 % de Black-Scholes avec le même σ.
#[test]
fn sabr_beta1_nu0_reduces_to_gbm() {
    let (s0, k, r, sigma, t) = (100.0, 100.0, 0.05, 0.20_f64, 1.0);

    // SABR avec β=1 et ν=0 → GBM avec σ=alpha=0.20
    let sabr = sabr_from_params(
        "X", s0, /* alpha */ sigma, /* beta */ 1.0, /* nu */ 0.0,
        /* rho */ 0.0, r,
    );

    let contract = european_call("X", k, t);
    let cfg = McConfig {
        n_paths: 500_000,
        seed: 42,
        steps_per_year: 100,
        rate: r,
    };

    let mc_price = price_gbm(&contract, &sabr, &cfg)
        .expect("SABR β=1 ν=0 pricing failed")
        .price;
    let bs_price = bs_call(s0, k, r, sigma, t);

    let rel_err = (mc_price - bs_price).abs() / bs_price;
    assert!(
        rel_err < 0.01,
        "SABR(β=1,ν=0) vs BS : MC={mc_price:.4}, BS={bs_price:.4}, err_rel={rel_err:.4} (seuil 1 %)"
    );
}

/// SABR-3 : Impact de ρ sur les appels OTM.
///
/// Avec ρ > 0 (corrélation positive spot/vol), la vol augmente quand le spot monte,
/// ce qui enrichit le prix des calls OTM (smile décalé vers la droite).
/// Avec ρ < 0 (corrélation négative), la vol monte quand le spot baisse,
/// créant un skew négatif typique des actions : les OTM calls sont moins chers.
///
/// On vérifie que la différence de prix est > 1 % du call ATM et que le signe
/// est correct (ρ > 0 → plus cher OTM).
#[test]
fn sabr_rho_impacts_otm_call_price() {
    let (s0, r, t) = (100.0, 0.05, 1.0);
    let k_otm = 110.0; // OTM : plus sensible à l'asymétrie du smile

    let sabr_pos_rho = sabr_from_params(
        "X", s0, /* alpha */ 0.30, /* beta */ 0.7,
        /* nu */ 0.5, // vol-de-vol élevée pour que ρ ait un effet visible
        /* rho */ 0.6, r,
    );
    let sabr_neg_rho = sabr_from_params(
        "X", s0, /* alpha */ 0.30, /* beta */ 0.7, /* nu */ 0.5,
        /* rho */ -0.6, r,
    );

    // Grille fine pour bien capter la dynamique SABR
    let n_steps = 200usize;
    let fine_grid: Vec<f64> = (0..=n_steps)
        .map(|i| i as f64 * t / n_steps as f64)
        .collect();

    let contract_otm = european_call("X", k_otm, t);
    let contract_atm = european_call("X", s0, t);
    let n_paths = 400_000;
    let seed = 42_u64;

    let paths_pos = sabr_pos_rho
        .simulate_paths(&fine_grid, n_paths, seed)
        .expect("SABR ρ>0 simulation failed");
    let paths_neg = sabr_neg_rho
        .simulate_paths(&fine_grid, n_paths, seed)
        .expect("SABR ρ<0 simulation failed");

    let price_otm_pos = price_on_paths(&contract_otm, &paths_pos, &fine_grid, r)
        .expect("price ρ>0 OTM")
        .price;
    let price_otm_neg = price_on_paths(&contract_otm, &paths_neg, &fine_grid, r)
        .expect("price ρ<0 OTM")
        .price;

    // Prix ATM de référence (pour calibrer le seuil de 1 %)
    let price_atm_pos = price_on_paths(&contract_atm, &paths_pos, &fine_grid, r)
        .expect("price ATM")
        .price;

    let diff = price_otm_pos - price_otm_neg;
    let threshold = 0.01 * price_atm_pos;

    // Direction : ρ > 0 → OTM call plus cher
    assert!(
        price_otm_pos > price_otm_neg,
        "SABR ρ>0 OTM call {price_otm_pos:.4} devrait être > ρ<0 OTM call {price_otm_neg:.4}"
    );

    // Magnitude : diff > 1 % du prix ATM
    assert!(
        diff > threshold,
        "SABR ρ impact OTM : diff={diff:.4}, seuil={threshold:.4} (1 % × ATM={price_atm_pos:.4})"
    );
}

// ============================================================================
// Tests Merton (3 tests)
// ============================================================================

/// Merton-1 : λ=0 → limite Black-Scholes.
///
/// Quand l'intensité des sauts est nulle (λ=0), le modèle de Merton se réduit
/// à un GBM avec la même volatilité de diffusion. Le prix doit être dans les 1 %
/// de Black-Scholes.
#[test]
fn merton_lambda_zero_reduces_to_bs() {
    let (s0, k, r, sigma, t) = (100.0, 100.0, 0.05, 0.20_f64, 1.0);

    let merton = merton_from_params(
        "X", s0, r, sigma, /* lambda */ 0.0, // pas de sauts
        /* mu_j */ 0.0, /* sigma_j */ 0.0,
    );

    let contract = european_call("X", k, t);
    let cfg = McConfig {
        n_paths: 500_000,
        seed: 42,
        steps_per_year: 100,
        rate: r,
    };

    let mc_price = price_gbm(&contract, &merton, &cfg)
        .expect("Merton λ=0 pricing failed")
        .price;
    let bs_price = bs_call(s0, k, r, sigma, t);

    let rel_err = (mc_price - bs_price).abs() / bs_price;
    assert!(
        rel_err < 0.01,
        "Merton(λ=0) vs BS : MC={mc_price:.4}, BS={bs_price:.4}, err_rel={rel_err:.4} (seuil 1 %)"
    );
}

/// Merton-2 : Prix MC vs formule fermée de Merton.
///
/// Avec des sauts modérés (λ=1 saut/an, μ_j=−0.10, σ_j=0.15), on compare
/// le prix MC au prix fermé de Merton (somme de BS pondérée Poisson).
/// La tolérance est de 3 % pour tenir compte du bruit MC.
#[test]
fn merton_mc_vs_closed_form() {
    let (s0, k, r, sigma, t) = (100.0, 100.0, 0.05, 0.20_f64, 1.0);
    let (lambda, mu_j, sigma_j) = (1.0, -0.10, 0.15);

    let merton = merton_from_params("X", s0, r, sigma, lambda, mu_j, sigma_j);

    let contract = european_call("X", k, t);
    let cfg = McConfig {
        n_paths: 600_000,
        seed: 42,
        steps_per_year: 100,
        rate: r,
    };

    let mc_price = price_gbm(&contract, &merton, &cfg)
        .expect("Merton MC pricing failed")
        .price;

    let closed_form = merton_closed_form(s0, k, r, sigma, t, lambda, mu_j, sigma_j);

    let rel_err = (mc_price - closed_form).abs() / closed_form;
    assert!(
        rel_err < 0.03,
        "Merton MC vs formule fermée : MC={mc_price:.4}, CF={closed_form:.4}, err_rel={rel_err:.4} (seuil 3 %)"
    );
}

/// Merton-3 : Impact des sauts sur les calls OTM.
///
/// Des sauts positifs (μ_j > 0) créent de la probabilité supplémentaire dans
/// la queue haute de la distribution → les calls OTM sont plus chers sous Merton
/// que sous GBM pur (même σ diffusion). Des sauts négatifs (μ_j < 0) enrichissent
/// plutôt les puts OTM et appauvrissent les calls OTM.
///
/// On vérifie le signe et la magnitude de l'impact pour des sauts positifs.
#[test]
fn merton_positive_jumps_increase_otm_call_price() {
    let (s0, r, sigma, t) = (100.0, 0.05, 0.15_f64, 1.0);
    let k_otm = 120.0; // call très OTM : sensible aux queues

    // Merton avec sauts positifs (μ_j > 0 → queue haute plus lourde)
    let (lambda, mu_j, sigma_j) = (3.0, 0.05, 0.10);
    let merton = merton_from_params("X", s0, r, sigma, lambda, mu_j, sigma_j);

    // GBM pur avec même σ diffusion (pas de sauts)
    let gbm_only = merton_from_params("X", s0, r, sigma, 0.0, 0.0, 0.0);

    let contract = european_call("X", k_otm, t);
    let cfg = McConfig {
        n_paths: 600_000,
        seed: 42,
        steps_per_year: 100,
        rate: r,
    };

    let price_merton = price_gbm(&contract, &merton, &cfg)
        .expect("Merton pricing failed")
        .price;
    let price_gbm_only = price_gbm(&contract, &gbm_only, &cfg)
        .expect("GBM pricing failed")
        .price;

    // Prix ATM pour calibrer le seuil de 1 %
    let contract_atm = european_call("X", s0, t);
    let price_atm = price_gbm(&contract_atm, &gbm_only, &cfg)
        .expect("ATM pricing failed")
        .price;

    let diff = price_merton - price_gbm_only;
    let threshold = 0.01 * price_atm;

    // Sauts positifs → call OTM plus cher
    assert!(
        price_merton > price_gbm_only,
        "Merton sauts positifs OTM call {price_merton:.4} devrait être > GBM {price_gbm_only:.4}"
    );

    // Magnitude : diff > 1 % du prix ATM
    assert!(
        diff > threshold,
        "Merton jump impact OTM : diff={diff:.4}, seuil={threshold:.4} (1 % × ATM={price_atm:.4})"
    );
}
