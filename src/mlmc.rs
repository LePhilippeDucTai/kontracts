//! Multilevel Monte-Carlo (jalon J18) — méthode de Giles (2008).
//!
//! # Principe
//!
//! Le Monte-Carlo standard sur un schéma d'Euler converge en O(ε⁻³) pour une
//! tolérance ε (erreur statistique O(N⁻¹ᐟ²) × biais de discrétisation O(Δt)).
//! Le **Multilevel Monte-Carlo** (MLMC) exploite une hiérarchie de grilles
//! temporelles de plus en plus fines pour ramener ce coût à O(ε⁻²).
//!
//! L'idée repose sur la **décomposition téléscopique** de l'espérance du payoff
//! sur la grille la plus fine `L` :
//!
//! ```text
//! E[P_L] = E[P_0] + Σ_{ℓ=1}^{L} E[P_ℓ − P_{ℓ−1}]
//! ```
//!
//! où `P_ℓ` est le payoff évalué sur une grille à `2^ℓ` pas. Chaque terme
//! `Y_ℓ = P_ℓ − P_{ℓ−1}` est estimé indépendamment, **mais** les deux
//! discrétisations partagent les mêmes incréments browniens (couplage assuré par
//! [`Simulator::simulate_level_pair`]). Ce couplage fait décroître rapidement la
//! variance `V_ℓ = Var(Y_ℓ)` (typiquement O(2⁻²ℓ) pour Euler), si bien que les
//! niveaux fins — coûteux — ne nécessitent que **peu** de trajectoires.
//!
//! # Allocation optimale (Giles)
//!
//! Pour une variance cible `ε²`, minimiser le coût total `Σ N_ℓ·C_ℓ` sous la
//! contrainte `Σ V_ℓ/N_ℓ = ε²` donne (multiplicateur de Lagrange) :
//!
//! ```text
//! N_ℓ ∝ √(V_ℓ / C_ℓ),   N_ℓ = ⌈ ε⁻² · √(V_ℓ/C_ℓ) · Σ_k √(V_k·C_k) ⌉
//! ```
//!
//! où `C_ℓ ≈ 2^ℓ` est le coût (nombre de pas) d'une trajectoire au niveau `ℓ`.
//!
//! # Pipeline
//!
//! 1. **Run pilote** : `pilot_paths` trajectoires par niveau → estime `V_ℓ` et
//!    `μ_ℓ = E[Y_ℓ]`.
//! 2. **Allocation** : calcule `N_ℓ` optimaux pour la variance cible.
//! 3. **Run principal** : simule les `N_ℓ` trajectoires supplémentaires par
//!    niveau, agrège `Q = Σ μ_ℓ` avec variance `Var(Q) = Σ V_ℓ/N_ℓ`.
//!
//! Le pricer reste **compositionnel et agnostique au modèle** : MLMC évalue le
//! même `Contract` que le pricer standard, via [`Simulator::simulate_level_pair`].

use crate::ast::Contract;
use crate::pricer::{present_value_pub, McConfig, PriceResult};
use crate::simulator::Simulator;
use crate::KontractError;

/// Configuration du Multilevel Monte-Carlo (jalon J18).
#[derive(Debug, Clone)]
pub struct MlmcConfig {
    /// Niveau le plus fin `L` (grille à `2^L` pas). Typiquement 5–10.
    pub n_levels: usize,
    /// Nombre de trajectoires du run pilote, par niveau (estimation de `V_ℓ`).
    pub pilot_paths: usize,
    /// Variance cible `ε²` de l'estimateur final (typiquement `1e-4`).
    pub target_variance: f64,
    /// Multiplicateur de coût par pas de temps (1 pour Euler).
    pub cost_per_step: usize,
}

impl Default for MlmcConfig {
    fn default() -> Self {
        MlmcConfig {
            n_levels: 6,
            pilot_paths: 2_000,
            target_variance: 1e-4,
            cost_per_step: 1,
        }
    }
}

/// Diagnostics détaillés d'un pricing MLMC (jalon J18).
#[derive(Debug, Clone)]
pub struct MlmcResult {
    /// Résultat de pricing agrégé (prix + IC 95 %).
    pub price: PriceResult,
    /// Espérances par niveau `μ_ℓ = E[Y_ℓ]` (Y₀ = P₀, Yℓ = Pℓ − Pℓ₋₁).
    pub level_means: Vec<f64>,
    /// Variances par niveau `V_ℓ = Var(Y_ℓ)` (estimées au run pilote).
    pub level_variances: Vec<f64>,
    /// Coûts par niveau `C_ℓ` (nombre de pas × `cost_per_step`).
    pub level_costs: Vec<usize>,
    /// Trajectoires par niveau effectivement utilisées (pilote + run principal),
    /// cohérent avec `total_cost` et la variance agrégée.
    pub level_paths: Vec<usize>,
    /// Coût total = `Σ_ℓ (pilot_paths + N_ℓ) · C_ℓ` (en pas de temps simulés).
    pub total_cost: u64,
}

const Z95: f64 = 1.959_963_984_540_054;

/// Prix d'un contrat par Multilevel Monte-Carlo (jalon J18).
///
/// Renvoie un [`PriceResult`] standard (compatible avec le reste du moteur).
/// Pour les diagnostics par niveau, utiliser [`price_mlmc_detailed`].
pub fn price_mlmc(
    contract: &Contract,
    model: &dyn Simulator,
    cfg: &McConfig,
    mlmc_cfg: &MlmcConfig,
) -> Result<PriceResult, KontractError> {
    Ok(price_mlmc_detailed(contract, model, cfg, mlmc_cfg)?.price)
}

/// Variante de [`price_mlmc`] renvoyant les diagnostics complets par niveau.
pub fn price_mlmc_detailed(
    contract: &Contract,
    model: &dyn Simulator,
    cfg: &McConfig,
    mlmc_cfg: &MlmcConfig,
) -> Result<MlmcResult, KontractError> {
    let l_max = mlmc_cfg.n_levels;
    let t_max = horizon(contract)?;

    // Coûts par niveau : C_0 = 1 pas, C_ℓ = 2^ℓ + 2^(ℓ−1) pas (fin + grossier).
    let costs: Vec<usize> = (0..=l_max)
        .map(|l| level_cost(l) * mlmc_cfg.cost_per_step.max(1))
        .collect();

    // ── 1. Run pilote : estime μ_ℓ et V_ℓ par niveau ────────────────────────
    let mut means = vec![0.0f64; l_max + 1];
    let mut variances = vec![0.0f64; l_max + 1];
    // Sommes courantes (pilote) pour fusionner avec le run principal.
    let mut sums = vec![0.0f64; l_max + 1];
    let mut sumsq = vec![0.0f64; l_max + 1];
    let mut counts = vec![0usize; l_max + 1];

    for (l, item) in variances.iter_mut().enumerate() {
        let ys = level_samples(contract, model, l, mlmc_cfg.pilot_paths, cfg, t_max)?;
        let (mean, var) = mean_var(&ys);
        means[l] = mean;
        *item = var;
        sums[l] = ys.iter().sum();
        sumsq[l] = ys.iter().map(|y| y * y).sum();
        counts[l] = ys.len();
    }

    // ── 2. Allocation optimale N_ℓ ───────────────────────────────────────────
    let extra = optimal_allocation(&variances, &costs, mlmc_cfg.target_variance);

    // ── 3. Run principal : trajectoires supplémentaires par niveau ───────────
    for (l, &n_extra) in extra.iter().enumerate() {
        if n_extra == 0 {
            continue;
        }
        // Graine décalée pour éviter de réutiliser le run pilote.
        let mut cfg_main = cfg.clone();
        cfg_main.seed = cfg.seed ^ 0xA5A5_5A5A_DEAD_BEEF;
        let ys = level_samples(contract, model, l, n_extra, &cfg_main, t_max)?;
        sums[l] += ys.iter().sum::<f64>();
        sumsq[l] += ys.iter().map(|y| y * y).sum::<f64>();
        counts[l] += ys.len();
    }

    // Fusion pilote + principal → moments par niveau définitifs.
    for l in 0..=l_max {
        let n = counts[l].max(1) as f64;
        let mean = sums[l] / n;
        means[l] = mean;
        variances[l] = if counts[l] > 1 {
            ((sumsq[l] - n * mean * mean) / (n - 1.0)).max(0.0)
        } else {
            0.0
        };
    }

    // ── Agrégation MLMC : Q = Σ μ_ℓ, Var(Q) = Σ V_ℓ/N_ℓ ─────────────────────
    let price: f64 = means.iter().sum();
    let var_q: f64 = (0..=l_max)
        .map(|l| {
            let n = counts[l].max(1) as f64;
            variances[l] / n
        })
        .sum();
    let std_error = var_q.max(0.0).sqrt();
    let ci95 = Z95 * std_error;
    let n_total: usize = counts.iter().sum();

    let total_cost: u64 = (0..=l_max)
        .map(|l| counts[l] as u64 * costs[l] as u64)
        .sum();

    let price_result = PriceResult {
        price,
        sample_std: std_error * (n_total.max(1) as f64).sqrt(),
        std_error,
        ci95_low: price - ci95,
        ci95_high: price + ci95,
        n_paths: n_total,
    };

    Ok(MlmcResult {
        price: price_result,
        level_means: means,
        level_variances: variances,
        level_costs: costs,
        // Nombre total de trajectoires par niveau effectivement utilisées par
        // l'estimateur (pilote + principal), cohérent avec `total_cost` et la
        // variance agrégée. L'allocation optimale brute (run principal seul) est
        // `extra` ci-dessus.
        level_paths: counts,
        total_cost,
    })
}

/// Estime la variance `V_ℓ = Var(P_ℓ − P_{ℓ−1})` au niveau `level` par un run
/// pilote de `n_pilot` trajectoires (jalon J18, fonction interne exposée pour
/// les tests).
pub fn estimate_variance_at_level(
    contract: &Contract,
    model: &dyn Simulator,
    level: usize,
    n_pilot: usize,
    cfg: &McConfig,
) -> Result<f64, KontractError> {
    let t_max = horizon(contract)?;
    let ys = level_samples(contract, model, level, n_pilot, cfg, t_max)?;
    Ok(mean_var(&ys).1)
}

/// Allocation optimale de Giles : `N_ℓ = ⌈ ε⁻² · √(V_ℓ/C_ℓ) · Σ_k √(V_k·C_k) ⌉`.
///
/// Minimise le coût total `Σ N_ℓ·C_ℓ` sous la contrainte `Σ V_ℓ/N_ℓ = ε²`.
/// Garantit au moins 1 trajectoire par niveau actif.
///
/// `variances` et `costs` doivent avoir la même longueur ; sinon on tronque à la
/// plus courte (robustesse — pas de panique d'indexation).
pub fn optimal_allocation(variances: &[f64], costs: &[usize], target_var: f64) -> Vec<usize> {
    let l = variances.len().min(costs.len());
    if l == 0 || target_var <= 0.0 {
        return vec![0; l];
    }

    // Σ_k √(V_k · C_k).
    let sum_sqrt_vc: f64 = (0..l)
        .map(|k| (variances[k].max(0.0) * costs[k].max(1) as f64).sqrt())
        .sum();

    (0..l)
        .map(|k| {
            let v = variances[k].max(0.0);
            let c = costs[k].max(1) as f64;
            if v <= 0.0 {
                return 1; // niveau sans variance : 1 trajectoire suffit.
            }
            let n = (sum_sqrt_vc * (v / c).sqrt()) / target_var;
            (n.ceil() as usize).max(1)
        })
        .collect()
}

// ────────────────────────────────────────────────────────────────────────────
// Helpers internes
// ────────────────────────────────────────────────────────────────────────────

/// Coût d'une trajectoire au niveau `ℓ` en nombre de pas de temps simulés.
///
/// Niveau 0 : 1 pas (fin seul). Niveau ℓ>0 : `2^ℓ` (fin) + `2^(ℓ−1)` (grossier).
fn level_cost(level: usize) -> usize {
    if level == 0 {
        1
    } else {
        (1usize << level) + (1usize << (level - 1))
    }
}

/// Échantillons `Y_ℓ = P_ℓ − P_{ℓ−1}` (différence fine − grossière) pour `n`
/// trajectoires couplées au niveau `level`. Le niveau 0 renvoie simplement `P_0`.
fn level_samples(
    contract: &Contract,
    model: &dyn Simulator,
    level: usize,
    n: usize,
    cfg: &McConfig,
    t_max: f64,
) -> Result<Vec<f64>, KontractError> {
    if n == 0 {
        return Ok(vec![]);
    }
    let seed = crate::simulator::mix(cfg.seed, level as u64);
    let pair = model.simulate_level_pair(t_max, level, n, seed)?;
    let (fine, coarse) = match pair {
        Some(p) => p,
        None => {
            return Err(KontractError::Unsupported(
                "ce simulateur ne supporte pas MLMC (simulate_level_pair)".into(),
            ))
        }
    };

    let mut ys = Vec::with_capacity(fine.len());
    for (i, fp) in fine.iter().enumerate() {
        let pv_fine = present_value_pub(contract, fp, fp.times(), cfg.rate)?;
        let pv_coarse = if level == 0 {
            0.0
        } else {
            let cp = &coarse[i];
            present_value_pub(contract, cp, cp.times(), cfg.rate)?
        };
        ys.push(pv_fine - pv_coarse);
    }
    Ok(ys)
}

/// Horizon temporel du contrat (date maximale), via le compilateur.
fn horizon(contract: &Contract) -> Result<f64, KontractError> {
    let plan = crate::compiler::compile(contract)?;
    Ok(plan.horizon)
}

/// Moyenne et variance d'échantillon (estimateur sans biais, n−1).
fn mean_var(xs: &[f64]) -> (f64, f64) {
    let n = xs.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let mean = xs.iter().sum::<f64>() / n as f64;
    let var = if n > 1 {
        xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n as f64 - 1.0)
    } else {
        0.0
    };
    (mean, var)
}
