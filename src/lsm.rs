//! Pricing d'options **américaines** par Longstaff-Schwartz (LSM) — jalon J17.
//!
//! Une option américaine confère le droit d'exercer un payoff à **n'importe
//! quelle** date d'un ensemble discret de dates d'exercice. La valeur dépend
//! d'une politique d'exercice optimale : à chaque date, le détenteur compare le
//! **payoff immédiat** à la **valeur de continuation** (l'espérance de la valeur
//! future conditionnelle à l'information courante). Cette espérance n'est pas
//! connue en MC ; Longstaff & Schwartz (2001) l'**estiment par régression**
//! polynomiale des cash-flows futurs actualisés sur une base de fonctions du
//! sous-jacent, en **backward induction**.
//!
//! # Architecture (cf. CLAUDE.md)
//!
//! L'algorithme respecte la séparation stricte des couches :
//!   - **aucune** modification de l'AST : pas de nouveau combinateur `american`.
//!     L'exercice américain n'est pas une primitive de l'algèbre mais un **mode
//!     d'exécution** du pricer (comme MC vs EDP), choisi via cette fonction
//!     dédiée — exactement comme `anytime` est documenté comme « first-touch,
//!     exercice optimal → J17 » dans le pricer ;
//!   - **compositionnalité préservée** : le payoff exercé reste un [`Contract`]
//!     arbitraire, évalué par la même brique [`crate::pricer`] (réduction en
//!     flux le long du path) ; LSM n'introduit aucun cas spécial par produit ;
//!   - **agnostique au modèle** : la simulation passe par le trait
//!     [`Simulator`], donc GBM, Heston, Dupire… (J11+) fonctionnent tels quels.
//!
//! # Algorithme
//!
//! ```text
//! V[i] = payoff(S[i, T])                       // valeur à la dernière date
//! pour t de T-dt à la première date d'exercice :
//!     immediate[i]   = payoff(S[i, t])
//!     # `cont_hat` (estimée par régression sur les paths ITM) sert UNIQUEMENT à
//!     # décider l'exercice ; la valeur transportée reste un cash-flow réalisé.
//!     cont_hat[i] = poly_fit(basis(S_itm), V_itm·disc)·basis(S[i,t])
//!     si immediate[i] > cont_hat[i] :  V[i] = immediate[i]   // exerce
//!     sinon                         :  V[i] = V[i]·disc       // continue (réalisé)
//! prix = mean(V[i] actualisé jusqu'à la première date)
//!
//! Transporter la valeur **réalisée** (et non l'estimation `cont_hat`) en cas de
//! continuation est ce qui évite le biais de sur-estimation in-sample du LSM.
//! ```

use rayon::prelude::*;

use crate::ast::Contract;
use crate::numerics;
use crate::observable::Path;
use crate::pricer::PriceResult;
use crate::simulator::Simulator;
use crate::KontractError;
use crate::McConfig;

/// Configuration de la régression Longstaff-Schwartz.
#[derive(Debug, Clone, Copy)]
pub struct LsmConfig {
    /// Nombre de fonctions de base polynomiales (degré + 1).
    /// `2` → {1, S}, `3` → {1, S, S²}, etc. Typiquement 2–3 (Longstaff-Schwartz).
    pub n_basis: usize,
}

impl Default for LsmConfig {
    fn default() -> Self {
        LsmConfig { n_basis: 3 }
    }
}

/// Price une option **américaine** par Longstaff-Schwartz.
///
/// `payoff` est le contrat exercé : sa valeur **à la date d'exercice** (flux
/// actualisés jusqu'à cette date) sert de payoff immédiat. Par exemple, un put
/// américain de strike `K` est `scale(max(K − S, 0), one(ccy))` — **sans** `when`,
/// puisque les dates d'exercice sont fournies séparément.
///
/// `exercise_dates` est l'ensemble (croissant, > 0) des dates où l'exercice est
/// permis. La dernière date est la maturité.
///
/// La simulation utilise `model` (n'importe quel [`Simulator`]) sur une grille
/// **incluant** toutes les dates d'exercice. Le taux `cfg.rate` est le taux
/// d'actualisation déterministe (et le drift risque-neutre du modèle).
pub fn price_american_lsm(
    contract: &Contract,
    exercise_dates: &[f64],
    model: &dyn Simulator,
    cfg: &McConfig,
    lsm_cfg: &LsmConfig,
) -> Result<PriceResult, KontractError> {
    if exercise_dates.is_empty() {
        return Err(KontractError::MalformedContract(
            "LSM : au moins une date d'exercice requise".into(),
        ));
    }
    if exercise_dates.windows(2).any(|w| w[1] <= w[0]) || exercise_dates[0] <= 0.0 {
        return Err(KontractError::MalformedContract(
            "LSM : dates d'exercice strictement croissantes et > 0 requises".into(),
        ));
    }
    if lsm_cfg.n_basis < 2 {
        return Err(KontractError::MalformedContract(
            "LSM : n_basis ≥ 2 requis (constante + linéaire)".into(),
        ));
    }

    // Grille de simulation : 0.0 puis toutes les dates d'exercice.
    let mut grid = vec![0.0];
    grid.extend_from_slice(exercise_dates);

    // Index de grille de chaque date d'exercice (décalé de 1 par le 0.0 initial).
    let ex_idx: Vec<usize> = (1..grid.len()).collect();

    let paths = model.simulate_paths(&grid, cfg.n_paths, cfg.seed)?;
    let asset = model.asset_name().to_string();

    let pvs = backward_induction(
        &paths,
        &grid,
        &ex_idx,
        contract,
        &asset,
        cfg.rate,
        lsm_cfg.n_basis,
    )?;

    Ok(crate::pricer::summarize_pvs(&pvs))
}

/// Backward induction LSM. Renvoie la valeur **actualisée à t = 0** de chaque path.
///
/// `ex_idx` sont les index (dans `grid`) des dates d'exercice, croissants.
fn backward_induction(
    paths: &[Path],
    grid: &[f64],
    ex_idx: &[usize],
    contract: &Contract,
    asset: &str,
    rate: f64,
    n_basis: usize,
) -> Result<Vec<f64>, KontractError> {
    let n_paths = paths.len();
    let n_ex = ex_idx.len();

    // Payoff immédiat de chaque path à chaque date d'exercice (valeur **à la date**).
    // payoff[d][i] = valeur du contrat exercé à la date ex_idx[d] sur le path i.
    let mut payoff = vec![vec![0.0f64; n_paths]; n_ex];
    for (d, &ti) in ex_idx.iter().enumerate() {
        let col: Result<Vec<f64>, KontractError> = paths
            .par_iter()
            .map(|p| value_at(contract, p, grid, ti, rate))
            .collect();
        payoff[d] = col?;
    }

    // Spot du sous-jacent à chaque date d'exercice (base de la régression).
    let mut spots = vec![vec![0.0f64; n_paths]; n_ex];
    for (d, &ti) in ex_idx.iter().enumerate() {
        let col: Result<Vec<f64>, KontractError> =
            paths.iter().map(|p| p.spot(asset, ti)).collect();
        spots[d] = col?;
    }

    // `value[i]` : valeur du contrat américain le long du path i, **mesurée à la
    // date d'exercice courante** au fil du backward. Initialisée à la dernière date.
    let mut value = payoff[n_ex - 1].clone();

    // Remonte de l'avant-dernière date d'exercice vers la première.
    for d in (0..n_ex - 1).rev() {
        let t_now = grid[ex_idx[d]];
        let t_next = grid[ex_idx[d + 1]];
        let disc = (-rate * (t_next - t_now)).exp();

        // Cibles de régression : valeur future actualisée d'un pas jusqu'à `t_now`.
        let discounted: Vec<f64> = value.iter().map(|v| v * disc).collect();

        // Régression sur les paths **dans la monnaie** (payoff immédiat > 0),
        // convention standard de Longstaff-Schwartz : réduit le bruit et améliore
        // l'ajustement là où la décision d'exercice est pertinente.
        let xs_itm: Vec<f64> = (0..n_paths)
            .filter(|&i| payoff[d][i] > 0.0)
            .map(|i| spots[d][i])
            .collect();
        let ys_itm: Vec<f64> = (0..n_paths)
            .filter(|&i| payoff[d][i] > 0.0)
            .map(|i| discounted[i])
            .collect();

        // Si trop peu de points ITM pour ajuster la base, personne n'exerce ici :
        // la valeur continue (actualisée) vers la date suivante.
        let coeffs = if xs_itm.len() >= n_basis {
            Some(poly_fit(&xs_itm, &ys_itm, n_basis)?)
        } else {
            None
        };

        for i in 0..n_paths {
            if payoff[d][i] > 0.0 {
                let continuation = match &coeffs {
                    Some(c) => poly_eval(c, spots[d][i]),
                    None => discounted[i],
                };
                // Exercice optimal : exerce si le payoff immédiat domine l'estimation
                // de continuation. Sinon, conserve la valeur future actualisée.
                if payoff[d][i] > continuation {
                    value[i] = payoff[d][i];
                } else {
                    value[i] = discounted[i];
                }
            } else {
                // Hors de la monnaie : pas d'exercice, on continue.
                value[i] = discounted[i];
            }
        }
    }

    // Actualisation de la première date d'exercice jusqu'à t = 0.
    let t_first = grid[ex_idx[0]];
    let disc0 = (-rate * t_first).exp();
    Ok(value.iter().map(|v| v * disc0).collect())
}

/// Valeur d'un contrat **exercé à l'instant `grid[ti]`** sur un path : on réduit
/// le contrat en flux acquis à `ti`, puis on actualise chaque flux **jusqu'à la
/// date d'exercice** `grid[ti]` (pas jusqu'à t = 0).
///
/// Pour un payoff vanille `scale(obs, one(ccy))`, le flux est unique et à `ti`,
/// donc la valeur vaut simplement `obs(S[ti])`. La forme générale supporte des
/// payoffs composés (and, give, scale imbriqués…) sans cas particulier.
fn value_at(
    contract: &Contract,
    path: &Path,
    grid: &[f64],
    ti: usize,
    rate: f64,
) -> Result<f64, KontractError> {
    let t_ex = grid[ti];
    let flows = crate::pricer::cashflows_pub(contract, ti, path)?;
    Ok(flows
        .into_iter()
        .map(|(amount, k)| amount * (-rate * (grid[k] - t_ex)).exp())
        .sum())
}

/// Ajuste un polynôme de degré `n_basis − 1` par moindres carrés (équations
/// normales `AᵀA c = Aᵀy`), résolues par élimination de Gauss avec pivot partiel.
///
/// Base : `{1, x, x², …, x^(n_basis−1)}`. Pour des `n_basis` petits (2–4) et des
/// régressions par date, le coût `O(n_basis³ + n·n_basis²)` est négligeable.
#[allow(clippy::needless_range_loop)] // indexation matricielle volontaire (AᵀA)
fn poly_fit(xs: &[f64], ys: &[f64], n_basis: usize) -> Result<Vec<f64>, KontractError> {
    let n = xs.len();
    let m = n_basis;

    // Matrice de Vandermonde implicite : on accumule directement AᵀA (m×m) et Aᵀy (m).
    let mut ata = vec![vec![0.0f64; m]; m];
    let mut aty = vec![0.0f64; m];

    // Pré-allouer le buffer des puissances une fois (réutilisé par itération).
    let mut powers = vec![1.0f64; m];

    for k in 0..n {
        // Puissances de x : [1, x, x², …]
        powers[0] = 1.0;
        for j in 1..m {
            powers[j] = powers[j - 1] * xs[k];
        }
        for r in 0..m {
            aty[r] += powers[r] * ys[k];
            for c in 0..m {
                ata[r][c] += powers[r] * powers[c];
            }
        }
    }

    numerics::solve_linear(ata, aty)
}

// solve_linear is centralized in numerics module

/// Évalue le polynôme `c[0] + c[1]·x + c[2]·x² + …` par schéma de Horner.
fn poly_eval(coeffs: &[f64], x: f64) -> f64 {
    coeffs.iter().rev().fold(0.0, |acc, &c| acc * x + c)
}
