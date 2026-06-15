//! Optimiseur global **CMA-ES** (Covariance Matrix Adaptation Evolution
//! Strategy) — jalon J22.
//!
//! CMA-ES (Hansen & Ostermeier, 2001) est une stratégie d'évolution **sans
//! gradient** : à chaque génération elle échantillonne une population
//! gaussienne `N(m, σ²C)`, sélectionne les meilleurs individus, puis adapte la
//! moyenne `m`, le pas global `σ` et la matrice de covariance `C` pour épouser la
//! géométrie locale de l'objectif. Deux propriétés en font l'outil de
//! **calibration** par excellence là où la descente de type trust-region
//! (J21-fast) échoue :
//!
//!   - **robustesse au bruit** : l'objectif Monte-Carlo est bruité ; les
//!     gradients par différences finies sont peu fiables. CMA-ES ne dérive
//!     jamais et reste stable.
//!   - **caractère global** : l'adaptation de covariance permet d'échapper aux
//!     minima locaux, fréquents sur les surfaces de calibration Heston/SABR.
//!
//! # Séparation des couches (cf. CLAUDE.md)
//!
//! Cet optimiseur est **agnostique au domaine** : il minimise une fonction
//! objectif `Fn(&[f64]) -> f64` arbitraire sur un pavé de contraintes. Il
//! n'a **aucune** connaissance d'un produit financier, d'un modèle ou d'un
//! pricer. Les fonctions de calibration (cf. [`crate::calibration`]) le
//! branchent sur des objectifs de reprise de prix — exactement comme le pricer
//! ne connaît que les combinateurs primitifs.
//!
//! # Référence
//!
//! N. Hansen, *« The CMA Evolution Strategy: A Tutorial »* (2016), dont sont
//! tirées les constantes de stratégie (`c_σ`, `d_σ`, `c_c`, `c_1`, `c_μ`).

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rand_distr::StandardNormal;
use rayon::prelude::*;

use crate::numerics;
use crate::simulator::mix;

/// Pavé de contraintes (box constraints) sur les paramètres optimisés.
///
/// Chaque coordonnée `i` est contrainte à `[lower[i], upper[i]]`. CMA-ES
/// échantillonne dans l'espace réel ; les points sont **projetés** sur le pavé
/// avant l'évaluation de l'objectif (clamp), ce qui garantit des paramètres
/// physiquement admissibles (ex. `v0 > 0`, `ρ ∈ (−1, 1)`, condition de Feller).
#[derive(Debug, Clone)]
pub struct Bounds {
    pub lower: Vec<f64>,
    pub upper: Vec<f64>,
}

impl Bounds {
    /// Construit un pavé à partir de bornes inférieures et supérieures.
    pub fn new(lower: Vec<f64>, upper: Vec<f64>) -> Self {
        Bounds { lower, upper }
    }

    /// Projette un point sur le pavé (clamp coordonnée par coordonnée).
    fn clamp(&self, x: &[f64]) -> Vec<f64> {
        x.iter()
            .zip(self.lower.iter())
            .zip(self.upper.iter())
            .map(|((&xi, &lo), &hi)| xi.clamp(lo, hi))
            .collect()
    }
}

/// Configuration de l'optimiseur CMA-ES.
#[derive(Debug, Clone)]
pub struct CmaesConfig {
    /// Taille de population `λ`. `None` → valeur par défaut `4 + ⌊3·ln n⌋`.
    pub population_size: Option<usize>,
    /// Pas global initial `σ₀` (échelle d'exploration, en unités de paramètre).
    pub sigma0: f64,
    /// Nombre maximum de générations.
    pub max_generations: usize,
    /// Tolérance d'arrêt sur l'étalement (`σ·max(D) < tol_x`).
    pub tol_x: f64,
    /// Tolérance d'arrêt sur la dispersion de l'objectif entre meilleures valeurs.
    pub tol_fun: f64,
    /// Graine RNG (reproductibilité bit-à-bit).
    pub seed: u64,
}

impl Default for CmaesConfig {
    fn default() -> Self {
        CmaesConfig {
            population_size: None,
            sigma0: 0.3,
            max_generations: 200,
            tol_x: 1e-9,
            tol_fun: 1e-12,
            seed: 42,
        }
    }
}

/// Résultat d'une optimisation CMA-ES.
#[derive(Debug, Clone)]
pub struct OptimizeResult {
    /// Meilleur jeu de paramètres trouvé (déjà projeté sur le pavé).
    pub best_params: Vec<f64>,
    /// Valeur de l'objectif au meilleur point.
    pub best_objective: f64,
    /// Nombre de générations effectuées.
    pub generations: usize,
    /// `true` si un critère de convergence (et non `max_generations`) a stoppé.
    pub converged: bool,
}

/// Constantes de stratégie CMA-ES dérivées de la dimension `n` et des poids.
///
/// Regroupées dans une struct pour garder le cœur de l'algorithme lisible et
/// éviter une dizaine de `let` épars (toutes constantes pour une dimension donnée).
struct Strategy {
    n: usize,
    mu: usize,
    weights: Vec<f64>,
    mu_eff: f64,
    c_sigma: f64,
    d_sigma: f64,
    c_c: f64,
    c_1: f64,
    c_mu: f64,
    chi_n: f64,
}

impl Strategy {
    fn new(n: usize, lambda: usize) -> Self {
        let mu = lambda / 2;

        // Poids préliminaires log-décroissants, puis normalisés à somme 1.
        let raw: Vec<f64> = (0..mu)
            .map(|i| (mu as f64 + 0.5).ln() - ((i + 1) as f64).ln())
            .collect();
        let sum: f64 = raw.iter().sum();
        let weights: Vec<f64> = raw.iter().map(|&w| w / sum).collect();

        // Masse de sélection effective μ_eff = 1 / Σ w_i².
        let mu_eff = 1.0 / weights.iter().map(|&w| w * w).sum::<f64>();

        let nf = n as f64;
        let c_sigma = (mu_eff + 2.0) / (nf + mu_eff + 5.0);
        let d_sigma = 1.0 + 2.0 * (((mu_eff - 1.0) / (nf + 1.0)).sqrt() - 1.0).max(0.0) + c_sigma;
        let c_c = (4.0 + mu_eff / nf) / (nf + 4.0 + 2.0 * mu_eff / nf);
        let c_1 = 2.0 / ((nf + 1.3).powi(2) + mu_eff);
        let c_mu = ((1.0 - c_1)
            .min(2.0 * (mu_eff - 2.0 + 1.0 / mu_eff) / ((nf + 2.0).powi(2) + mu_eff)))
        .max(0.0);
        // Espérance de ||N(0, I)|| (approximation usuelle).
        let chi_n = nf.sqrt() * (1.0 - 1.0 / (4.0 * nf) + 1.0 / (21.0 * nf * nf));

        Strategy {
            n,
            mu,
            weights,
            mu_eff,
            c_sigma,
            d_sigma,
            c_c,
            c_1,
            c_mu,
            chi_n,
        }
    }
}

/// Minimise `objective` sur le pavé `bounds` par CMA-ES, depuis le point `x0`.
///
/// `objective` est évaluée en **parallèle** (rayon) sur la population à chaque
/// génération : c'est le point chaud (un pricing Monte-Carlo par individu).
/// L'échantillonnage gaussien est, lui, séquentiel et déterministe (graine
/// dérivée par `mix(seed, génération)`) → résultat reproductible bit-à-bit.
///
/// Les points échantillonnés sont projetés sur `bounds` avant évaluation, donc
/// `best_params` est toujours admissible.
pub fn cmaes_minimize<F>(
    objective: F,
    x0: &[f64],
    bounds: &Bounds,
    cfg: &CmaesConfig,
) -> OptimizeResult
where
    F: Fn(&[f64]) -> f64 + Sync,
{
    let n = x0.len();
    let lambda = cfg
        .population_size
        .unwrap_or_else(|| 4 + (3.0 * (n as f64).ln()).floor() as usize)
        .max(4);
    let st = Strategy::new(n, lambda);

    // État dynamique de la stratégie (réassigné à chaque génération).
    let mut mean = x0.to_vec();
    let mut sigma = cfg.sigma0;
    let mut p_sigma = vec![0.0f64; n];
    let mut p_c = vec![0.0f64; n];
    // Covariance C, initialisée à l'identité.
    let mut cov: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
        .collect();

    // Meilleur point global rencontré (projeté), maintenu hors de l'état CMA-ES.
    let clamped0 = bounds.clamp(x0);
    let mut best_x = clamped0.clone();
    let mut best_f = objective(&clamped0);
    let mut converged = false;
    let mut generations = 0usize;

    // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
    // Boucle de générations CMA-ES : récurrence séquentielle (l'état m, σ, C, p_σ,
    // p_c d'une génération dépend de la précédente). L'algèbre matricielle interne
    // (eigendécomposition, rang-1/rang-μ) est un noyau numérique.
    for gen in 0..cfg.max_generations {
        generations = gen + 1;

        // Factorisation spectrale C = B·diag(d²)·Bᵀ → B (vecteurs propres), d (√λ).
        let (eigvals, eigvecs) = numerics::jacobi_eigen(&cov);
        // Planchage des valeurs propres pour robustesse (C reste SPD malgré l'arrondi).
        let d: Vec<f64> = eigvals.iter().map(|&e| e.max(1e-18).sqrt()).collect();

        // Échantillonnage de λ individus : z ~ N(0,I), y = B·D·z ~ N(0,C), x = m + σy.
        // Séquentiel et déterministe (graine par génération) ; évaluation parallèle.
        let mut rng = ChaCha8Rng::seed_from_u64(mix(cfg.seed, gen as u64 + 1));
        let samples: Vec<(Vec<f64>, Vec<f64>)> = (0..lambda)
            .map(|_| {
                let z: Vec<f64> = (0..n).map(|_| rng.sample(StandardNormal)).collect();
                let y = b_d_z(&eigvecs, &d, &z);
                let x: Vec<f64> = mean
                    .iter()
                    .zip(y.iter())
                    .map(|(&m, &yi)| m + sigma * yi)
                    .collect();
                (x, y)
            })
            .collect();

        // Évaluation parallèle de l'objectif sur les points **projetés**.
        let fitness: Vec<f64> = samples
            .par_iter()
            .map(|(x, _)| objective(&bounds.clamp(x)))
            .collect();

        // Tri des indices par fitness croissante (minimisation).
        let mut order: Vec<usize> = (0..lambda).collect();
        order.sort_by(|&a, &b| {
            fitness[a]
                .partial_cmp(&fitness[b])
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Mise à jour du meilleur global (sur le point projeté).
        let best_idx = order[0];
        if fitness[best_idx] < best_f {
            best_f = fitness[best_idx];
            best_x = bounds.clamp(&samples[best_idx].0);
        }

        // Moyenne pondérée des y des μ meilleurs : y_w = Σ w_i y_{i:λ}.
        let y_w: Vec<f64> = (0..n)
            .map(|coord| {
                (0..st.mu)
                    .map(|i| st.weights[i] * samples[order[i]].1[coord])
                    .sum()
            })
            .collect();

        // Nouvelle moyenne : m ← m + σ·y_w (déplacement dans l'espace réel).
        let new_mean: Vec<f64> = mean
            .iter()
            .zip(y_w.iter())
            .map(|(&m, &yw)| m + sigma * yw)
            .collect();

        // Chemin d'évolution du pas : p_σ ← (1-c_σ)p_σ + √(c_σ(2-c_σ)μ_eff)·C^{-1/2}·y_w.
        let c_inv_sqrt_yw = c_inv_sqrt_apply(&eigvecs, &d, &y_w);
        let cs_factor = (st.c_sigma * (2.0 - st.c_sigma) * st.mu_eff).sqrt();
        p_sigma = p_sigma
            .iter()
            .zip(c_inv_sqrt_yw.iter())
            .map(|(&ps, &v)| (1.0 - st.c_sigma) * ps + cs_factor * v)
            .collect();
        let p_sigma_norm = norm(&p_sigma);

        // Heaviside h_σ : freine la mise à jour rang-1 quand le pas progresse vite.
        let denom = (1.0 - (1.0 - st.c_sigma).powi(2 * (gen as i32 + 1))).sqrt();
        let h_sigma = if p_sigma_norm / denom < (1.4 + 2.0 / (st.n as f64 + 1.0)) * st.chi_n {
            1.0
        } else {
            0.0
        };

        // Chemin d'évolution de la covariance : p_c ← (1-c_c)p_c + h_σ·√(...)·y_w.
        let cc_factor = (st.c_c * (2.0 - st.c_c) * st.mu_eff).sqrt();
        p_c = p_c
            .iter()
            .zip(y_w.iter())
            .map(|(&pc, &yw)| (1.0 - st.c_c) * pc + h_sigma * cc_factor * yw)
            .collect();

        // Mise à jour de la covariance (rang-1 + rang-μ) — noyau numérique.
        cov = update_covariance(&cov, &p_c, &samples, &order, &st, h_sigma);

        // Mise à jour du pas global : σ ← σ·exp((c_σ/d_σ)(||p_σ||/χ_n − 1)).
        sigma *= ((st.c_sigma / st.d_sigma) * (p_sigma_norm / st.chi_n - 1.0)).exp();

        mean = new_mean;

        // Critères d'arrêt : étalement effondré ou dispersion de fitness négligeable.
        let d_max = d.iter().cloned().fold(0.0f64, f64::max);
        let f_spread = (fitness[order[st.mu.min(lambda - 1)]] - fitness[order[0]]).abs();
        if sigma * d_max < cfg.tol_x || f_spread < cfg.tol_fun {
            converged = true;
            break;
        }
        if !sigma.is_finite() {
            break;
        }
    }

    OptimizeResult {
        best_params: best_x,
        best_objective: best_f,
        generations,
        converged,
    }
}

/// Produit `y = B·D·z` où `B` (vecteurs propres en colonnes) et `D = diag(d)`.
///
/// `eigvecs[k]` est la colonne `k` de `B` ; donc `y_i = Σ_k B_{ik} d_k z_k`.
fn b_d_z(eigvecs: &[Vec<f64>], d: &[f64], z: &[f64]) -> Vec<f64> {
    let n = z.len();
    // d_k z_k pré-calculé.
    let dz: Vec<f64> = d.iter().zip(z.iter()).map(|(&dk, &zk)| dk * zk).collect();
    (0..n)
        .map(|i| (0..n).map(|k| eigvecs[k][i] * dz[k]).sum())
        .collect()
}

/// Applique `C^{-1/2}·v = B·D^{-1}·Bᵀ·v` (utilisé pour le chemin de pas `p_σ`).
fn c_inv_sqrt_apply(eigvecs: &[Vec<f64>], d: &[f64], v: &[f64]) -> Vec<f64> {
    let n = v.len();
    // w_k = (Bᵀ v)_k / d_k = (Σ_i B_{ik} v_i) / d_k.
    let w: Vec<f64> = (0..n)
        .map(|k| {
            eigvecs[k]
                .iter()
                .zip(v.iter())
                .map(|(&b, &vi)| b * vi)
                .sum::<f64>()
                / d[k]
        })
        .collect();
    // result_i = Σ_k B_{ik} w_k.
    (0..n)
        .map(|i| (0..n).map(|k| eigvecs[k][i] * w[k]).sum())
        .collect()
}

/// Norme euclidienne d'un vecteur.
fn norm(v: &[f64]) -> f64 {
    v.iter().map(|&x| x * x).sum::<f64>().sqrt()
}

/// Met à jour la matrice de covariance `C` (combinaison rang-1 + rang-μ).
///
/// `C ← (1−c₁−c_μ)C + c₁(p_c p_cᵀ + δ·C) + c_μ Σ w_i y_i y_iᵀ`
/// avec la correction de Heaviside `δ = (1−h_σ)·c_c(2−c_c)`.
fn update_covariance(
    cov: &[Vec<f64>],
    p_c: &[f64],
    samples: &[(Vec<f64>, Vec<f64>)],
    order: &[usize],
    st: &Strategy,
    h_sigma: f64,
) -> Vec<Vec<f64>> {
    let n = st.n;
    let delta = (1.0 - h_sigma) * st.c_c * (2.0 - st.c_c);
    let decay = 1.0 - st.c_1 - st.c_mu;

    // Chaque entrée C_{ij} se recompose indépendamment (construction immutable).
    (0..n)
        .map(|i| {
            (0..n)
                .map(|j| {
                    let rank1 = st.c_1 * (p_c[i] * p_c[j] + delta * cov[i][j]);
                    let rank_mu: f64 = (0..st.mu)
                        .map(|k| {
                            let y = &samples[order[k]].1;
                            st.weights[k] * y[i] * y[j]
                        })
                        .sum::<f64>()
                        * st.c_mu;
                    decay * cov[i][j] + rank1 + rank_mu
                })
                .collect()
        })
        .collect()
}
