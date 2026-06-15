//! Backtesting et validation de modèle (jalon J23).
//!
//! Couche d'**orchestration** au sommet de l'architecture : ce module ne connaît
//! aucun produit nommé et n'ajoute **aucune** primitive à l'AST. Il rejoue le
//! moteur de pricing (couche pricer) le long de séries historiques / de
//! trajectoires réalisées, puis en dérive des diagnostics de validation.
//!
//! Trois familles de diagnostics, alignées sur le critère du jalon
//! (« prix historiques vs modèle, stability » → « validation PnL réel ») :
//!
//!   1. **PnL de réplication par couverture en delta** ([`backtest_delta_hedge`]) :
//!      le test décisif de cohérence d'un modèle. On vend l'option à son prix
//!      modèle, on couvre en delta de façon auto-financée le long de trajectoires
//!      réalisées (tirées par le **simulateur du moteur** lui-même), et on mesure
//!      l'erreur de réplication terminale. Si le modèle et la couverture sont
//!      cohérents, le PnL moyen ≈ 0 et son écart-type décroît en `√dt` quand on
//!      rééquilibre plus souvent. C'est la **validation par PnL réalisé**.
//!
//!   2. **Erreur modèle vs marché** ([`model_vs_market`]) : statistiques d'écart
//!      (biais, RMSE, erreur max, erreur relative) entre une série de prix modèle
//!      et les prix de marché observés — « prix historiques vs modèle ».
//!
//!   3. **Stabilité** ([`stability`]) : variation jour-le-jour d'une série
//!      (erreurs, paramètres calibrés, prix) : dérive, volatilité des variations,
//!      saut maximal.
//!
//! La fonction [`historical_model_prices`] relie (1)/(2) au moteur : elle rejoue
//! le pricer Monte-Carlo sur une séquence d'observations, en restant **agnostique
//! au produit et au modèle** (elle ne reçoit qu'un constructeur `(Contract, Gbm)`).

use rayon::prelude::*;

use crate::numerics::{black_scholes_call, black_scholes_put, norm_cdf};
use crate::pricer::{price_gbm, McConfig};
use crate::simulator::Gbm;
use crate::{Contract, KontractError};

/// Delta Black-Scholes analytique d'un call/put vanille (rendement de dividende nul).
///
/// `tau` est le temps restant jusqu'à l'échéance. À l'échéance (`tau ≤ 0`) le delta
/// dégénère en indicatrice de fin dans la monnaie.
pub fn bs_delta(s: f64, k: f64, tau: f64, r: f64, sigma: f64, is_call: bool) -> f64 {
    if tau <= 0.0 || sigma <= 0.0 {
        // Delta à l'échéance : 1{S>K} (call) ou −1{S<K} (put).
        return match (is_call, s >= k) {
            (true, true) => 1.0,
            (true, false) => 0.0,
            (false, true) => 0.0,
            (false, false) => -1.0,
        };
    }
    let d1 = ((s / k).ln() + (r + 0.5 * sigma * sigma) * tau) / (sigma * tau.sqrt());
    let nd1 = norm_cdf(d1);
    if is_call {
        nd1
    } else {
        nd1 - 1.0
    }
}

/// Prix Black-Scholes d'un call/put selon le drapeau `is_call`.
fn bs_price(s: f64, k: f64, tau: f64, r: f64, sigma: f64, is_call: bool) -> f64 {
    if is_call {
        black_scholes_call(s, k, tau, r, sigma)
    } else {
        black_scholes_put(s, k, tau, r, sigma)
    }
}

/// Erreur de réplication d'une couverture en delta auto-financée, discrète, le
/// long d'**une** trajectoire réalisée échantillonnée sur une grille uniforme
/// `[0, T]` (`spots[0] = S_0`, `spots[n−1] = S_T`, pas `dt`).
///
/// Convention : on encaisse la prime `V_0 = BS(S_0)` à `t = 0` et on constitue
/// le portefeuille répliquant `(cash, shares)` ; à chaque date on accumule les
/// intérêts puis on rééquilibre au delta courant ; à l'échéance on liquide les
/// titres et on règle le payoff. Le résultat est
/// `valeur_portefeuille(T) − payoff(T)` : positif = couverture excédentaire.
///
/// Pour une couverture continue exacte ce serait 0 ; en discret il subsiste une
/// erreur de moyenne nulle et de variance ∝ `dt`.
pub fn delta_hedge_error(
    spots: &[f64],
    dt: f64,
    strike: f64,
    rate: f64,
    sigma: f64,
    is_call: bool,
) -> Result<f64, KontractError> {
    let n = spots.len();
    if n < 2 {
        return Err(KontractError::MalformedContract(
            "delta_hedge_error: au moins 2 points de trajectoire requis".into(),
        ));
    }

    let t_max = (n - 1) as f64 * dt;
    let s0 = spots[0];
    let d0 = bs_delta(s0, strike, t_max, rate, sigma, is_call);
    let v0 = bs_price(s0, strike, t_max, rate, sigma, is_call);
    // État initial du portefeuille répliquant : (cash, titres détenus).
    let init = (v0 - d0 * s0, d0);

    // Avance auto-financée : intérêts puis rééquilibrage (sauf à l'échéance).
    let growth = (rate * dt).exp();
    let (cash, shares) = spots
        .iter()
        .enumerate()
        .skip(1)
        .fold(init, |(cash, shares), (i, &s)| {
            let cash = cash * growth;
            if i < n - 1 {
                let tau = (n - 1 - i) as f64 * dt;
                let d = bs_delta(s, strike, tau, rate, sigma, is_call);
                (cash - (d - shares) * s, d)
            } else {
                (cash, shares)
            }
        });

    let s_t = spots[n - 1];
    let payoff = if is_call {
        (s_t - strike).max(0.0)
    } else {
        (strike - s_t).max(0.0)
    };
    Ok(cash + shares * s_t - payoff)
}

/// Diagnostics d'un backtest de couverture en delta sur un ensemble de
/// trajectoires réalisées.
#[derive(Debug, Clone, PartialEq)]
pub struct HedgeBacktestReport {
    /// PnL de réplication moyen (≈ 0 si modèle et couverture sont cohérents).
    pub mean_pnl: f64,
    /// Écart-type empirique du PnL de réplication.
    pub std_pnl: f64,
    /// Racine de l'erreur quadratique moyenne du PnL (`√(moyenne(PnL²))`).
    pub rmse: f64,
    /// Prime initiale `V_0` (échelle de référence pour juger l'erreur relative).
    pub premium: f64,
    /// Nombre de trajectoires.
    pub n_paths: usize,
    /// Nombre de rééquilibrages (pas de temps) par trajectoire.
    pub n_rebalances: usize,
}

/// Backtest de **couverture en delta** : tire `n_paths` trajectoires réalisées
/// via le **simulateur du moteur** (`model`), couvre chacune au delta analytique
/// du même modèle, et agrège la distribution du PnL de réplication terminal.
///
/// Le drift et la volatilité de couverture sont ceux du modèle (`model.mu`,
/// `model.sigma`) : on couvre avec le modèle qui « génère la réalité », donc le
/// PnL moyen doit valoir ≈ 0 — c'est précisément le test de **validation par
/// PnL réalisé** du jalon. Augmenter `n_steps` réduit l'écart-type en `√dt`.
pub fn backtest_delta_hedge(
    model: &Gbm,
    strike: f64,
    maturity: f64,
    is_call: bool,
    n_steps: usize,
    n_paths: usize,
    seed: u64,
) -> Result<HedgeBacktestReport, KontractError> {
    if n_steps == 0 || n_paths == 0 {
        return Err(KontractError::MalformedContract(
            "backtest_delta_hedge: n_steps et n_paths doivent être > 0".into(),
        ));
    }
    let rate = model.mu;
    let sigma = model.sigma;
    let dt = maturity / n_steps as f64;

    // Grille uniforme [0, T] à n_steps+1 points (S_0 inclus).
    let grid: Vec<f64> = (0..=n_steps)
        .map(|k| maturity * k as f64 / n_steps as f64)
        .collect();

    let paths = model.simulate(&grid, n_paths, seed)?;

    // PnL par trajectoire (évaluation parallèle, indépendante de l'ordre).
    let pnls = paths
        .outer_iter()
        .into_par_iter()
        .map(|row| {
            let spots = row.to_vec();
            delta_hedge_error(&spots, dt, strike, rate, sigma, is_call)
        })
        .collect::<Result<Vec<f64>, _>>()?;

    let n = pnls.len() as f64;
    let mean_pnl = pnls.iter().sum::<f64>() / n;
    let var = pnls.iter().map(|p| (p - mean_pnl).powi(2)).sum::<f64>() / n;
    let rmse = (pnls.iter().map(|p| p * p).sum::<f64>() / n).sqrt();
    let premium = bs_price(model.s0, strike, maturity, rate, sigma, is_call);

    Ok(HedgeBacktestReport {
        mean_pnl,
        std_pnl: var.sqrt(),
        rmse,
        premium,
        n_paths,
        n_rebalances: n_steps,
    })
}

/// Statistiques d'écart entre prix modèle et prix de marché observés.
#[derive(Debug, Clone, PartialEq)]
pub struct PricingErrorReport {
    /// Erreur moyenne signée `moyenne(modèle − marché)` = biais.
    pub bias: f64,
    /// Erreur absolue moyenne `moyenne(|modèle − marché|)`.
    pub mean_abs_error: f64,
    /// Racine de l'erreur quadratique moyenne.
    pub rmse: f64,
    /// Erreur absolue maximale.
    pub max_abs_error: f64,
    /// Erreur relative absolue moyenne `moyenne(|modèle − marché| / |marché|)`
    /// (les observations de prix de marché nul sont ignorées).
    pub mean_relative_error: f64,
    /// Nombre d'observations.
    pub n: usize,
}

/// Compare une série de prix modèle à une série de prix de marché de même longueur.
pub fn model_vs_market(
    model_prices: &[f64],
    market_prices: &[f64],
) -> Result<PricingErrorReport, KontractError> {
    if model_prices.len() != market_prices.len() {
        return Err(KontractError::MalformedContract(
            "model_vs_market: séries de longueurs différentes".into(),
        ));
    }
    if model_prices.is_empty() {
        return Err(KontractError::MalformedContract(
            "model_vs_market: série vide".into(),
        ));
    }

    let n = model_prices.len();
    let errors: Vec<f64> = model_prices
        .iter()
        .zip(market_prices.iter())
        .map(|(m, q)| m - q)
        .collect();

    let bias = errors.iter().sum::<f64>() / n as f64;
    let mean_abs_error = errors.iter().map(|e| e.abs()).sum::<f64>() / n as f64;
    let rmse = (errors.iter().map(|e| e * e).sum::<f64>() / n as f64).sqrt();
    let max_abs_error = errors.iter().map(|e| e.abs()).fold(0.0_f64, f64::max);

    // Erreur relative : moyenne sur les seules observations de marché non nulles.
    let (rel_sum, rel_count) =
        errors
            .iter()
            .zip(market_prices.iter())
            .fold((0.0_f64, 0_usize), |(acc, cnt), (e, q)| {
                if q.abs() > 0.0 {
                    (acc + (e / q).abs(), cnt + 1)
                } else {
                    (acc, cnt)
                }
            });
    let mean_relative_error = if rel_count > 0 {
        rel_sum / rel_count as f64
    } else {
        f64::NAN
    };

    Ok(PricingErrorReport {
        bias,
        mean_abs_error,
        rmse,
        max_abs_error,
        mean_relative_error,
        n,
    })
}

/// Diagnostics de stabilité d'une série temporelle.
#[derive(Debug, Clone, PartialEq)]
pub struct StabilityReport {
    /// Moyenne des valeurs.
    pub value_mean: f64,
    /// Écart-type des valeurs.
    pub value_std: f64,
    /// Dérive moyenne (moyenne des variations `xₜ − xₜ₋₁`).
    pub change_mean: f64,
    /// Volatilité des variations jour-le-jour (écart-type des `xₜ − xₜ₋₁`).
    pub change_std: f64,
    /// Saut absolu maximal entre deux observations consécutives.
    pub max_abs_change: f64,
    /// Nombre de valeurs.
    pub n: usize,
}

/// Mesure la stabilité d'une série (variation jour-le-jour) : utile pour juger
/// la stabilité d'une série de paramètres calibrés ou d'erreurs de pricing dans
/// le temps. Une série stable a `change_std` et `max_abs_change` faibles.
pub fn stability(series: &[f64]) -> Result<StabilityReport, KontractError> {
    let n = series.len();
    if n < 2 {
        return Err(KontractError::MalformedContract(
            "stability: au moins 2 valeurs requises".into(),
        ));
    }

    let value_mean = series.iter().sum::<f64>() / n as f64;
    let value_std =
        (series.iter().map(|x| (x - value_mean).powi(2)).sum::<f64>() / n as f64).sqrt();

    let changes: Vec<f64> = series.windows(2).map(|w| w[1] - w[0]).collect();
    let m = changes.len() as f64;
    let change_mean = changes.iter().sum::<f64>() / m;
    let change_std = (changes
        .iter()
        .map(|c| (c - change_mean).powi(2))
        .sum::<f64>()
        / m)
        .sqrt();
    let max_abs_change = changes.iter().map(|c| c.abs()).fold(0.0_f64, f64::max);

    Ok(StabilityReport {
        value_mean,
        value_std,
        change_mean,
        change_std,
        max_abs_change,
        n,
    })
}

/// Rejoue le moteur Monte-Carlo sur une séquence de `n_obs` observations
/// historiques et renvoie la série des prix modèle.
///
/// `build(i)` fournit le `(Contract, Gbm)` **tel qu'à la date d'observation `i`**
/// (typiquement : spot du jour, maturité résiduelle décroissante). La fonction
/// reste **agnostique au produit et au modèle** : elle ne fait qu'appeler le
/// pricer compositionnel. Le résultat alimente [`model_vs_market`] / [`stability`].
pub fn historical_model_prices<F>(
    n_obs: usize,
    build: F,
    cfg: &McConfig,
) -> Result<Vec<f64>, KontractError>
where
    F: Fn(usize) -> (Contract, Gbm) + Sync,
{
    (0..n_obs)
        .into_par_iter()
        .map(|i| {
            let (contract, model) = build(i);
            price_gbm(&contract, &model, cfg).map(|r| r.price)
        })
        .collect()
}
