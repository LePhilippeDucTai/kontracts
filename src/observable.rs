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

use crate::ast::Observable;
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
        }
    }
}
