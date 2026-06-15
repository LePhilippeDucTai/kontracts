//! Évaluation des observables sur un path simulé (jalon J2).
//!
//! L'AST ([`crate::ast::Observable`]) est purement descriptif. C'est ici que
//! vit la **logique numérique** : étant donné une trajectoire de marché et un
//! pas de temps, on réduit un observable à un `f64`.
//!
//! Un [`Path`] est une trajectoire discrète : une grille temporelle commune et,
//! pour chaque sous-jacent, la suite de ses prix spot sur cette grille. Le
//! simulateur (J3) produira ces paths ; ici on les construit à la main pour les
//! tests.

use std::collections::HashMap;

use crate::ast::{Condition, Observable};
use crate::KontractError;

/// Une trajectoire de marché discrète, partagée par tous les sous-jacents.
#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    /// Grille temporelle (en années), strictement croissante.
    times: Vec<f64>,
    /// Prix spot par sous-jacent ; chaque vecteur a la longueur de `times`.
    spots: HashMap<String, Vec<f64>>,
}

impl Path {
    /// Crée un path vide sur la grille temporelle donnée.
    pub fn new(times: Vec<f64>) -> Self {
        Path {
            times,
            spots: HashMap::new(),
        }
    }

    /// Ajoute la trajectoire d'un sous-jacent.
    ///
    /// Renvoie une erreur si la longueur ne correspond pas à la grille.
    pub fn with_asset(
        mut self,
        name: impl Into<String>,
        values: Vec<f64>,
    ) -> Result<Self, KontractError> {
        if values.len() != self.times.len() {
            return Err(KontractError::InconsistentPath(format!(
                "{} valeurs pour {} dates",
                values.len(),
                self.times.len()
            )));
        }
        self.spots.insert(name.into(), values);
        Ok(self)
    }

    /// Nombre de pas de temps.
    pub fn len(&self) -> usize {
        self.times.len()
    }

    /// `true` si la grille est vide.
    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }

    /// Grille temporelle.
    pub fn times(&self) -> &[f64] {
        &self.times
    }

    /// Série complète des spots d'un sous-jacent sur toute la grille.
    pub fn spot_series(&self, name: &str) -> Result<&[f64], KontractError> {
        self.spots
            .get(name)
            .map(|v| v.as_slice())
            .ok_or_else(|| KontractError::UnknownAsset(name.to_string()))
    }

    /// Prix spot d'un sous-jacent au pas `t`.
    pub fn spot(&self, name: &str, t: usize) -> Result<f64, KontractError> {
        let series = self
            .spots
            .get(name)
            .ok_or_else(|| KontractError::UnknownAsset(name.to_string()))?;
        series
            .get(t)
            .copied()
            .ok_or(KontractError::StepOutOfRange(t))
    }
}

impl Observable {
    /// Évalue l'observable sur `path` au pas de temps `t`.
    pub fn eval(&self, path: &Path, t: usize) -> Result<f64, KontractError> {
        match self {
            Observable::Const(x) => Ok(*x),
            Observable::Spot(name) => path.spot(name, t),
            Observable::Neg(a) => Ok(-a.eval(path, t)?),
            Observable::Add(a, b) => Ok(a.eval(path, t)? + b.eval(path, t)?),
            Observable::Sub(a, b) => Ok(a.eval(path, t)? - b.eval(path, t)?),
            Observable::Mul(a, b) => Ok(a.eval(path, t)? * b.eval(path, t)?),
            Observable::Div(a, b) => Ok(a.eval(path, t)? / b.eval(path, t)?),
            Observable::Max(a, b) => Ok(a.eval(path, t)?.max(b.eval(path, t)?)),
            Observable::Min(a, b) => Ok(a.eval(path, t)?.min(b.eval(path, t)?)),
            Observable::Average {
                obs,
                from_year,
                to_year,
            } => {
                // t_now doubles as a bounds-check for t even when to_year is Some.
                let t_now = *path
                    .times()
                    .get(t)
                    .ok_or(KontractError::StepOutOfRange(t))?;
                let t_from = from_year.unwrap_or(0.0);
                let t_to = to_year.unwrap_or(t_now);
                if t_from > t_to + 1e-12 {
                    return Err(KontractError::MalformedContract(format!(
                        "fenêtre moyenne invalide : from {t_from} > to {t_to}"
                    )));
                }
                let vals: Result<Vec<f64>, _> = path
                    .times()
                    .iter()
                    .enumerate()
                    .filter(|(_, &ti)| ti >= t_from - 1e-12 && ti <= t_to + 1e-12)
                    .map(|(i, _)| obs.eval(path, i))
                    .collect();
                let vals = vals?;
                if vals.is_empty() {
                    return Err(KontractError::MalformedContract(format!(
                        "fenêtre moyenne vide : [{from_year:?}, {to_year:?}]"
                    )));
                }
                Ok(vals.iter().sum::<f64>() / vals.len() as f64)
            }
            Observable::RunningMax(obs) => (0..=t)
                .map(|i| obs.eval(path, i))
                .try_fold(f64::NEG_INFINITY, |acc, v| v.map(|x| acc.max(x))),
            Observable::RunningMin(obs) => (0..=t)
                .map(|i| obs.eval(path, i))
                .try_fold(f64::INFINITY, |acc, v| v.map(|x| acc.min(x))),
        }
    }
}

impl Condition {
    /// Évalue la condition sur `path` au pas de temps `t` (jalon J6).
    ///
    /// `At(t0)` est vraie dès que la date courante atteint `t0` ; les
    /// comparaisons s'appuient sur l'évaluation des observables.
    pub fn eval(&self, path: &Path, t: usize) -> Result<bool, KontractError> {
        match self {
            Condition::Bool(b) => Ok(*b),
            Condition::At(t0) => {
                let now = *path
                    .times()
                    .get(t)
                    .ok_or(KontractError::StepOutOfRange(t))?;
                Ok(now + 1e-12 >= *t0)
            }
            Condition::Ge(a, b) => Ok(a.eval(path, t)? >= b.eval(path, t)?),
            Condition::Gt(a, b) => Ok(a.eval(path, t)? > b.eval(path, t)?),
            Condition::Le(a, b) => Ok(a.eval(path, t)? <= b.eval(path, t)?),
            Condition::Lt(a, b) => Ok(a.eval(path, t)? < b.eval(path, t)?),
            Condition::And(a, b) => Ok(a.eval(path, t)? && b.eval(path, t)?),
            Condition::Or(a, b) => Ok(a.eval(path, t)? || b.eval(path, t)?),
            Condition::Not(a) => Ok(!a.eval(path, t)?),
        }
    }
}
