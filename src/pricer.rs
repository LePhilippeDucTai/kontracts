//! Pricer Monte-Carlo compositionnel (jalon J5).
//!
//! Le pricer ne connaît **aucun** produit nommé : il réduit récursivement un
//! [`Contract`] en une liste de flux `(montant, date)` le long de chaque
//! trajectoire, actualise, puis moyenne sur les trajectoires.
//!
//! Sémantique des combinateurs (sous-ensemble du jalon J5) :
//!   - `zero`        → aucun flux ;
//!   - `one(ccy)`    → un flux unitaire à la date d'acquisition courante ;
//!   - `give(c)`     → flux de `c` négativés ;
//!   - `and(a, b)`   → union des flux ;
//!   - `scale(o, c)` → chaque flux de `c` multiplié par `o` **évalué à la date
//!     de ce flux** (sémantique indépendante de l'imbrication) ;
//!   - `when(at(t), c)` → `c` acquis à la date `t`.
//!
//! Les barrières (`until`, `anytime`) et le choix (`or`) arrivent au jalon J6 :
//! ici ils renvoient [`KontractError::Unsupported`].

use rayon::prelude::*;

use crate::ast::{Condition, Contract};
use crate::compiler::{compile, Plan};
use crate::observable::Path;
use crate::simulator::Simulator;
use crate::variance_reduction::VarianceReductionConfig;
use crate::KontractError;

/// Paramètres de la simulation Monte-Carlo.
#[derive(Debug, Clone)]
pub struct McConfig {
    /// Nombre de trajectoires.
    pub n_paths: usize,
    /// Graine du RNG (reproductibilité).
    pub seed: u64,
    /// Résolution de la grille en présence de barrière (pas par an).
    pub steps_per_year: usize,
    /// Taux d'actualisation déterministe `r` (sert aussi de drift risque-neutre).
    pub rate: f64,
    /// Configuration de réduction de variance (jalon J15).
    /// `None` → comportement identique à J1–J14 (rétrocompatible).
    pub variance_reduction: Option<VarianceReductionConfig>,
}

impl Default for McConfig {
    fn default() -> Self {
        McConfig {
            n_paths: 100_000,
            seed: 42,
            steps_per_year: 50,
            rate: 0.0,
            variance_reduction: None,
        }
    }
}

/// Résultat d'un pricing, enrichi des diagnostics Monte-Carlo (jalon J5b).
#[derive(Debug, Clone, PartialEq)]
pub struct PriceResult {
    /// Prix (moyenne actualisée des flux sur les trajectoires).
    pub price: f64,
    /// Écart-type empirique des valeurs actualisées par trajectoire.
    pub sample_std: f64,
    /// Erreur standard de l'estimateur : `sample_std / √n`.
    pub std_error: f64,
    /// Borne basse de l'intervalle de confiance à 95 % (`price − 1.96·SE`).
    pub ci95_low: f64,
    /// Borne haute de l'intervalle de confiance à 95 % (`price + 1.96·SE`).
    pub ci95_high: f64,
    /// Nombre de trajectoires utilisées.
    pub n_paths: usize,
}

/// Quantile normal à 95 % (bilatéral).
const Z95: f64 = 1.959_963_984_540_054;

impl PriceResult {
    /// Estime le nombre de trajectoires nécessaires pour atteindre une demi-largeur
    /// d'intervalle de confiance à 95 % égale à `tol`.
    ///
    /// `n ≈ (1.96 · σ / tol)²`, où `σ` est l'écart-type empirique courant.
    pub fn paths_for_tolerance(&self, tol: f64) -> usize {
        if tol <= 0.0 || self.sample_std == 0.0 {
            return 0;
        }
        (Z95 * self.sample_std / tol).powi(2).ceil() as usize
    }
}

/// Price un contrat sous un [`Simulator`] quelconque (GBM par défaut).
///
/// Le nom historique (`price_gbm`) est conservé pour compatibilité, mais le
/// pricer ne dépend plus que de l'interface [`Simulator`] (jalon J11) : n'importe
/// quel modèle (Heston, Dupire… en J12+) peut être passé via `&dyn Simulator`.
///
/// Si `cfg.variance_reduction` est défini (jalon J15), applique les techniques
/// antithétiques et/ou de variable de contrôle avant de retourner le résultat.
pub fn price_gbm(
    contract: &Contract,
    model: &dyn Simulator,
    cfg: &McConfig,
) -> Result<PriceResult, KontractError> {
    let plan = compile(contract)?;
    let grid = plan.time_grid(cfg.steps_per_year);

    // ── Réduction de variance (J15) ─────────────────────────────────────────
    if let Some(vr) = &cfg.variance_reduction {
        return price_gbm_with_vr(contract, model, cfg, &grid, vr);
    }

    // ── Chemin standard (J1–J14, rétrocompatible) ───────────────────────────
    let paths = model.simulate_paths(&grid, cfg.n_paths, cfg.seed)?;
    price_on_paths(contract, &paths, &grid, cfg.rate)
}

/// Implémentation interne de `price_gbm` avec réduction de variance active (J15).
///
/// Deux techniques orthogonales :
/// - **Antithétiques** : n_paths/2 paires (Z, −Z) → moyenne des deux estimateurs.
/// - **Variable de contrôle** : call ATM GBM connu analytiquement (Black-Scholes).
fn price_gbm_with_vr(
    contract: &Contract,
    model: &dyn Simulator,
    cfg: &McConfig,
    grid: &[f64],
    vr: &VarianceReductionConfig,
) -> Result<PriceResult, KontractError> {
    // Tenter les trajectoires antithétiques si activées ; sinon fallback standard.
    if vr.use_antithetic {
        let n_half = cfg.n_paths / 2;
        if let Some((bases, antis)) = model.simulate_antithetic_paths(grid, n_half, cfg.seed)? {
            return if vr.use_control_variate {
                price_antithetic_with_cv(contract, model, grid, cfg, &bases, &antis)
            } else {
                price_antithetic_only(contract, &bases, &antis, grid, cfg.rate)
            };
        }
    }

    // Fallback : simulateur sans antithétique ou antithétique désactivée.
    let paths = model.simulate_paths(grid, cfg.n_paths, cfg.seed)?;
    if vr.use_control_variate {
        apply_cv_on_paths(contract, model, &paths, grid, cfg)
    } else {
        price_on_paths(contract, &paths, grid, cfg.rate)
    }
}

/// Applique la variable de contrôle (call ATM) sur des trajectoires déjà simulées.
fn apply_cv_on_paths(
    contract: &Contract,
    model: &dyn Simulator,
    paths: &[crate::observable::Path],
    grid: &[f64],
    cfg: &McConfig,
) -> Result<PriceResult, KontractError> {
    use crate::ast::{at, konst, one, scale, spot, when};
    use crate::variance_reduction::{black_scholes_call, price_control_variate_on_paths};

    let (s0, sigma) = model.gbm_params().unwrap_or((100.0, 0.2));
    let t_mat = *grid.last().unwrap_or(&1.0);
    let asset = model.asset_name();
    let ctrl_call = when(
        at(t_mat),
        scale((spot(asset) - konst(s0)).max(konst(0.0)), one("USD")),
    );
    let bs_price = black_scholes_call(s0, s0, t_mat, cfg.rate, sigma);
    price_control_variate_on_paths(contract, &ctrl_call, bs_price, 1.0, paths, grid, cfg.rate)
}

/// Price un contrat sur des trajectoires antithétiques **sans** variable de contrôle.
fn price_antithetic_only(
    contract: &Contract,
    bases: &[Path],
    antis: &[Path],
    grid: &[f64],
    rate: f64,
) -> Result<PriceResult, KontractError> {
    use crate::variance_reduction::price_antithetic_on_paths;
    price_antithetic_on_paths(contract, bases, antis, grid, rate)
}

/// Price un contrat sur des trajectoires antithétiques **avec** variable de contrôle.
fn price_antithetic_with_cv(
    contract: &Contract,
    model: &dyn Simulator,
    grid: &[f64],
    cfg: &McConfig,
    bases: &[Path],
    antis: &[Path],
) -> Result<PriceResult, KontractError> {
    use crate::ast::{at, konst, one, scale, spot, when};
    use crate::variance_reduction::{
        apply_control_variate, black_scholes_call, price_antithetic_on_paths,
    };

    let (s0, sigma) = model.gbm_params().unwrap_or((100.0, 0.2));
    let t_mat = *grid.last().unwrap_or(&1.0);
    let asset = model.asset_name();

    let ctrl_call = when(
        at(t_mat),
        scale((spot(asset) - konst(s0)).max(konst(0.0)), one("USD")),
    );
    let bs_price = black_scholes_call(s0, s0, t_mat, cfg.rate, sigma);

    let res_anti = price_antithetic_on_paths(contract, bases, antis, grid, cfg.rate)?;
    let ctrl_anti = price_antithetic_on_paths(&ctrl_call, bases, antis, grid, cfg.rate)?;

    let corrected_price = apply_control_variate(res_anti.price, ctrl_anti.price, bs_price, 1.0);
    let n = bases.len() + antis.len();
    let std_error = res_anti.sample_std / (n as f64).sqrt();
    let ci95 = Z95 * std_error;

    Ok(PriceResult {
        price: corrected_price,
        sample_std: res_anti.sample_std,
        std_error,
        ci95_low: corrected_price - ci95,
        ci95_high: corrected_price + ci95,
        n_paths: n,
    })
}

/// Évalue un contrat sur des trajectoires **déjà simulées** (parallèle par path).
///
/// Brique de base partagée par [`price_gbm`] et [`price_batch_gbm`] : permet de
/// réutiliser une même simulation pour plusieurs contrats.
pub fn price_on_paths(
    contract: &Contract,
    paths: &[Path],
    grid: &[f64],
    rate: f64,
) -> Result<PriceResult, KontractError> {
    let pvs = paths
        .par_iter()
        .map(|p| present_value(contract, p, grid, rate))
        .collect::<Result<Vec<f64>, KontractError>>()?;
    Ok(summarize(&pvs))
}

/// Price un **portefeuille** de contrats sous un même modèle GBM.
///
/// Optimisation clé (jalon J9c) : on compile tous les contrats, on construit une
/// grille temporelle **unifiée** (union des dates, fine si une barrière existe),
/// on simule les trajectoires **une seule fois**, puis on évalue chaque contrat
/// sur ces trajectoires partagées en parallèle (rayon). Pricer 100+ contrats ne
/// coûte donc qu'une simulation + des évaluations vectorisées.
pub fn price_batch_gbm(
    contracts: &[Contract],
    model: &dyn Simulator,
    cfg: &McConfig,
) -> Result<Vec<PriceResult>, KontractError> {
    if contracts.is_empty() {
        return Ok(vec![]);
    }

    // Plan unifié : union des dates, horizon max, grille fine si une barrière.
    let (fixed_dates, horizon, needs_fine_grid) = contracts
        .iter()
        .map(compile)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .fold(
            (Vec::new(), 0.0_f64, false),
            |(mut dates, h, fine), plan| {
                dates.extend(plan.fixed_dates);
                (dates, h.max(plan.horizon), fine | plan.needs_fine_grid)
            },
        );
    let merged = Plan {
        assets: Vec::new(),
        fixed_dates,
        horizon,
        needs_fine_grid,
    };
    let grid = merged.time_grid(cfg.steps_per_year);

    // Simulation unique, partagée par tous les contrats.
    let paths = model.simulate_paths(&grid, cfg.n_paths, cfg.seed)?;

    // Évaluation parallèle au niveau des contrats (boucle interne séquentielle).
    contracts
        .par_iter()
        .map(|c| {
            let pvs = paths
                .iter()
                .map(|p| present_value(c, p, &grid, cfg.rate))
                .collect::<Result<Vec<f64>, KontractError>>()?;
            Ok(summarize(&pvs))
        })
        .collect()
}

/// Agrège les valeurs actualisées par trajectoire en prix + diagnostics MC.
///
/// Alias public(crate) pour les modules de réduction de variance (J15+).
pub(crate) fn summarize_pvs(pvs: &[f64]) -> PriceResult {
    summarize(pvs)
}

fn summarize(pvs: &[f64]) -> PriceResult {
    let n = pvs.len();
    let price = pvs.iter().sum::<f64>() / n.max(1) as f64;

    // Variance d'échantillon (estimateur sans biais, n − 1).
    let sample_std = if n > 1 {
        let var = pvs.iter().map(|x| (x - price).powi(2)).sum::<f64>() / (n as f64 - 1.0);
        var.sqrt()
    } else {
        0.0
    };
    let std_error = if n > 0 {
        sample_std / (n as f64).sqrt()
    } else {
        0.0
    };

    PriceResult {
        price,
        sample_std,
        std_error,
        ci95_low: price - Z95 * std_error,
        ci95_high: price + Z95 * std_error,
        n_paths: n,
    }
}

/// Valeur actualisée d'un contrat sur une trajectoire individuelle.
///
/// Exposé publiquement pour la réduction de variance (J15+) : calcul du β optimal
/// par trajectoire, tests, etc.
pub fn present_value_pub(
    contract: &Contract,
    path: &Path,
    grid: &[f64],
    rate: f64,
) -> Result<f64, KontractError> {
    present_value(contract, path, grid, rate)
}

/// Valeur actualisée (à `t = 0`) d'un contrat sur une trajectoire.
fn present_value(
    contract: &Contract,
    path: &Path,
    grid: &[f64],
    rate: f64,
) -> Result<f64, KontractError> {
    let flows = cashflows(contract, 0, path)?;
    Ok(flows
        .into_iter()
        .map(|(amount, t_idx)| amount * (-rate * grid[t_idx]).exp())
        .sum())
}

/// Réduit un contrat en flux `(montant, index_de_date)` le long d'une trajectoire.
///
/// Exposé `pub(crate)` pour le pricing américain LSM (jalon J17), qui réduit le
/// payoff exercé à une date donnée en flux puis les actualise jusqu'à cette date.
pub(crate) fn cashflows_pub(
    contract: &Contract,
    t_idx: usize,
    path: &Path,
) -> Result<Vec<(f64, usize)>, KontractError> {
    cashflows(contract, t_idx, path)
}

/// Réduit un contrat en flux `(montant, index_de_date)` le long d'une trajectoire.
///
/// `t_idx` est l'index de la **date d'acquisition courante** du sous-contrat.
/// Les dates sont des index dans la grille de `path` ; l'actualisation est
/// appliquée par [`present_value`].
fn cashflows(
    contract: &Contract,
    t_idx: usize,
    path: &Path,
) -> Result<Vec<(f64, usize)>, KontractError> {
    match contract {
        Contract::Zero => Ok(vec![]),
        Contract::One(_) => Ok(vec![(1.0, t_idx)]),
        Contract::Give(c) => cashflows(c, t_idx, path)
            .map(|flows| flows.into_iter().map(|(amt, ti)| (-amt, ti)).collect()),
        Contract::And(a, b) => {
            let mut flows = cashflows(a, t_idx, path)?;
            flows.extend(cashflows(b, t_idx, path)?);
            Ok(flows)
        }
        Contract::Scale(obs, c) => cashflows(c, t_idx, path)?
            .into_iter()
            // L'observable est échantillonné à la date du flux qu'il met à l'échelle.
            .map(|(amt, ti)| obs.eval(path, ti).map(|scale| (amt * scale, ti)))
            .collect(),
        // `when` : acquiert `c` à la **première** activation de la condition
        // (date `at`, ou premier franchissement de barrière), à partir de `t_idx`.
        Contract::When(cond, c) => match first_activation(cond, path, t_idx)? {
            Some(k) => cashflows(c, k, path),
            None => Ok(vec![]),
        },
        // `anytime` : style américain, approximé en **premier franchissement**
        // (first-touch). L'exercice réellement optimal relève du jalon J17 (LSM).
        Contract::Anytime(cond, c) => match first_activation(cond, path, t_idx)? {
            Some(k) => cashflows(c, k, path),
            None => Ok(vec![]),
        },
        // `until` : knock-out. On garde les flux de `c` **strictement antérieurs**
        // à la première activation de la condition ; les autres sont annulés.
        Contract::Until(cond, c) => {
            let flows = cashflows(c, t_idx, path)?;
            match first_activation(cond, path, t_idx)? {
                Some(k) => Ok(flows.into_iter().filter(|(_, ti)| *ti < k).collect()),
                None => Ok(flows),
            }
        }
        // `or` : choix optimal du détenteur → arbitrage d'exercice (jalon J17, LSM).
        Contract::Or(_, _) => Err(KontractError::Unsupported("or (jalon J17, LSM)".into())),
    }
}

/// Premier pas de temps `>= start` où la condition est vraie sur cette trajectoire.
fn first_activation(
    cond: &Condition,
    path: &Path,
    start: usize,
) -> Result<Option<usize>, KontractError> {
    (start..path.len())
        .find_map(|t| match cond.eval(path, t) {
            Err(e) => Some(Err(e)),
            Ok(true) => Some(Ok(t)),
            Ok(false) => None,
        })
        .transpose()
}
