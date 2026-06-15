//! Compilateur : AST → plan de calcul (jalon J4).
//!
//! Le compilateur ne price rien. Il **analyse** statiquement un [`Contract`] pour
//! produire un [`Plan`] : le strict nécessaire pour piloter le simulateur et le
//! pricer.
//!
//! Trois informations sont extraites :
//!   - **assets** : les sous-jacents référencés (quoi simuler) ;
//!   - **dates fixes** : les instants d'acquisition explicites (`when(at(t), …)`) ;
//!   - **présence de barrière** : une condition *dépendante du prix*
//!     (`>=`, `<`, …) sous un `when`/`until`/`anytime` impose une grille de
//!     monitoring fine — d'où le drapeau [`Plan::needs_fine_grid`].
//!
//! À partir du plan, [`Plan::time_grid`] matérialise la grille temporelle de
//! simulation : juste les dates fixes si le contrat est « européen », ou une
//! grille dense jusqu'à l'horizon en présence d'une barrière.

use std::collections::BTreeSet;

use crate::ast::{Condition, Contract, Observable};
use crate::KontractError;

/// Plan de calcul extrait d'un contrat.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// Sous-jacents référencés, triés et dédupliqués.
    pub assets: Vec<String>,
    /// Dates d'acquisition explicites (`at(t)`), triées et dédupliquées.
    pub fixed_dates: Vec<f64>,
    /// Dernier instant pertinent du contrat (max des `at(t)`), ou `0.0`.
    pub horizon: f64,
    /// `true` si une barrière (condition dépendante du prix) doit être monitorée.
    pub needs_fine_grid: bool,
}

impl Plan {
    /// Matérialise la grille temporelle de simulation.
    ///
    /// - Toujours : `0.0`, les dates fixes, et l'horizon.
    /// - Si une barrière est présente : une subdivision dense de `[0, horizon]`
    ///   à la résolution `steps_per_year`.
    pub fn time_grid(&self, steps_per_year: usize) -> Vec<f64> {
        let mut pts = vec![0.0];
        pts.extend_from_slice(&self.fixed_dates);
        pts.push(self.horizon);

        if self.needs_fine_grid && self.horizon > 0.0 && steps_per_year > 0 {
            let n = (self.horizon * steps_per_year as f64).ceil() as usize;
            pts.extend((1..=n).map(|k| self.horizon * (k as f64) / (n as f64)));
        }

        sort_dedup(pts)
    }
}

/// Compile un contrat en plan de calcul.
pub fn compile(contract: &Contract) -> Result<Plan, KontractError> {
    let mut acc = Accumulator::default();
    walk_contract(contract, &mut acc)?;

    let horizon = acc
        .dates
        .iter()
        .copied()
        .fold(0.0_f64, |a, b| if b > a { b } else { a });

    Ok(Plan {
        assets: acc.assets.into_iter().collect(),
        fixed_dates: sort_dedup(acc.dates),
        horizon,
        needs_fine_grid: acc.needs_fine_grid,
    })
}

/// État mutable de la traversée.
#[derive(Default)]
struct Accumulator {
    assets: BTreeSet<String>,
    dates: Vec<f64>,
    needs_fine_grid: bool,
}

fn walk_contract(c: &Contract, acc: &mut Accumulator) -> Result<(), KontractError> {
    match c {
        Contract::Zero | Contract::One(_) => Ok(()),
        Contract::Give(inner) => walk_contract(inner, acc),
        Contract::And(a, b) | Contract::Or(a, b) => {
            walk_contract(a, acc)?;
            walk_contract(b, acc)
        }
        Contract::Scale(obs, inner) => {
            walk_observable(obs, acc);
            walk_contract(inner, acc)
        }
        Contract::When(cond, inner)
        | Contract::Anytime(cond, inner)
        | Contract::Until(cond, inner) => {
            walk_condition(cond, acc)?;
            if condition_is_price_dependent(cond) {
                acc.needs_fine_grid = true;
            }
            walk_contract(inner, acc)
        }
    }
}

fn walk_observable(o: &Observable, acc: &mut Accumulator) {
    match o {
        Observable::Const(_) => {}
        Observable::Spot(name) => {
            acc.assets.insert(name.clone());
        }
        Observable::Neg(a) => walk_observable(a, acc),
        Observable::Add(a, b)
        | Observable::Sub(a, b)
        | Observable::Mul(a, b)
        | Observable::Div(a, b)
        | Observable::Max(a, b)
        | Observable::Min(a, b) => {
            walk_observable(a, acc);
            walk_observable(b, acc);
        }
    }
}

fn walk_condition(cond: &Condition, acc: &mut Accumulator) -> Result<(), KontractError> {
    match cond {
        Condition::Bool(_) => Ok(()),
        Condition::At(t) => {
            if !t.is_finite() || *t < 0.0 {
                return Err(KontractError::MalformedContract(format!(
                    "date d'acquisition invalide : {t}"
                )));
            }
            acc.dates.push(*t);
            Ok(())
        }
        Condition::Ge(a, b) | Condition::Gt(a, b) | Condition::Le(a, b) | Condition::Lt(a, b) => {
            walk_observable(a, acc);
            walk_observable(b, acc);
            Ok(())
        }
        Condition::And(a, b) | Condition::Or(a, b) => {
            walk_condition(a, acc)?;
            walk_condition(b, acc)
        }
        Condition::Not(a) => walk_condition(a, acc),
    }
}

/// Une condition est « dépendante du prix » si elle contient une comparaison
/// d'observables — par opposition aux conditions purement temporelles (`at`).
fn condition_is_price_dependent(cond: &Condition) -> bool {
    match cond {
        Condition::Bool(_) | Condition::At(_) => false,
        Condition::Ge(..) | Condition::Gt(..) | Condition::Le(..) | Condition::Lt(..) => true,
        Condition::And(a, b) | Condition::Or(a, b) => {
            condition_is_price_dependent(a) || condition_is_price_dependent(b)
        }
        Condition::Not(a) => condition_is_price_dependent(a),
    }
}

/// Trie un vecteur de temps et supprime les quasi-doublons (tolérance 1e-12).
fn sort_dedup(mut v: Vec<f64>) -> Vec<f64> {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    v
}
