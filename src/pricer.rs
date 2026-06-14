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
}

impl Default for McConfig {
    fn default() -> Self {
        McConfig {
            n_paths: 100_000,
            seed: 42,
            steps_per_year: 50,
            rate: 0.0,
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
pub fn price_gbm(
    contract: &Contract,
    model: &dyn Simulator,
    cfg: &McConfig,
) -> Result<PriceResult, KontractError> {
    let plan = compile(contract)?;
    let grid = plan.time_grid(cfg.steps_per_year);
    let paths = model.simulate_paths(&grid, cfg.n_paths, cfg.seed)?;
    price_on_paths(contract, &paths, &grid, cfg.rate)
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
    let mut fixed_dates = Vec::new();
    let mut horizon = 0.0_f64;
    let mut needs_fine_grid = false;
    for c in contracts {
        let plan = compile(c)?;
        fixed_dates.extend(plan.fixed_dates);
        horizon = horizon.max(plan.horizon);
        needs_fine_grid |= plan.needs_fine_grid;
    }
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
        Contract::Give(c) => {
            let mut flows = cashflows(c, t_idx, path)?;
            for f in &mut flows {
                f.0 = -f.0;
            }
            Ok(flows)
        }
        Contract::And(a, b) => {
            let mut flows = cashflows(a, t_idx, path)?;
            flows.extend(cashflows(b, t_idx, path)?);
            Ok(flows)
        }
        Contract::Scale(obs, c) => {
            let mut flows = cashflows(c, t_idx, path)?;
            for f in &mut flows {
                // L'observable est échantillonné à la date du flux qu'il met à l'échelle.
                f.0 *= obs.eval(path, f.1)?;
            }
            Ok(flows)
        }
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
    for t in start..path.len() {
        if cond.eval(path, t)? {
            return Ok(Some(t));
        }
    }
    Ok(None)
}
