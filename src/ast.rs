//! AST — l'algèbre des contrats (jalon J1).
//!
//! Ce module ne contient que des **types purs et sérialisables**. Aucune logique
//! numérique, aucune notion de path ou de pricing : l'AST décrit *quoi* est un
//! contrat, jamais *comment* on l'évalue (cf. CLAUDE.md).
//!
//! Trois familles de types :
//!   - [`Observable`] : un processus à valeur réelle (prix, arithmétique).
//!   - [`Condition`]  : un processus booléen (date, barrière, comparaison).
//!   - [`Contract`]   : les combinateurs primitifs à la Peyton Jones.

use std::ops::{Add, Div, Mul, Neg, Not, Sub};

use serde::{Deserialize, Serialize};

use crate::KontractError;

/// Devise d'un flux unitaire (p.ex. `"USD"`, `"EUR"`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Currency(pub String);

impl Currency {
    /// Construit une devise depuis n'importe quoi de convertible en `String`.
    pub fn new(code: impl Into<String>) -> Self {
        Currency(code.into())
    }
}

/// Observable : un processus à valeur réelle, décrit symboliquement.
///
/// L'évaluation concrète sur un path simulé est le rôle du jalon J2 ; ici on ne
/// fait que **décrire** l'expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Observable {
    /// Constante déterministe.
    Const(f64),
    /// Prix spot d'un sous-jacent identifié par son nom (p.ex. `"AAPL"`).
    Spot(String),
    /// Opposé arithmétique.
    Neg(Box<Observable>),
    /// Somme.
    Add(Box<Observable>, Box<Observable>),
    /// Différence.
    Sub(Box<Observable>, Box<Observable>),
    /// Produit.
    Mul(Box<Observable>, Box<Observable>),
    /// Quotient.
    Div(Box<Observable>, Box<Observable>),
    /// Maximum point à point (utile pour les payoffs `max(S - K, 0)`).
    Max(Box<Observable>, Box<Observable>),
    /// Minimum point à point.
    Min(Box<Observable>, Box<Observable>),
}

impl Observable {
    /// `max(self, other)` — payoff plancher.
    pub fn max(self, other: Observable) -> Observable {
        Observable::Max(Box::new(self), Box::new(other))
    }

    /// `min(self, other)` — payoff plafond.
    pub fn min(self, other: Observable) -> Observable {
        Observable::Min(Box::new(self), Box::new(other))
    }

    /// Condition `self >= other`.
    pub fn ge(self, other: Observable) -> Condition {
        Condition::Ge(self, other)
    }

    /// Condition `self > other`.
    pub fn gt(self, other: Observable) -> Condition {
        Condition::Gt(self, other)
    }

    /// Condition `self <= other`.
    pub fn le(self, other: Observable) -> Condition {
        Condition::Le(self, other)
    }

    /// Condition `self < other`.
    pub fn lt(self, other: Observable) -> Condition {
        Condition::Lt(self, other)
    }
}

impl Add for Observable {
    type Output = Observable;
    fn add(self, rhs: Observable) -> Observable {
        Observable::Add(Box::new(self), Box::new(rhs))
    }
}

impl Sub for Observable {
    type Output = Observable;
    fn sub(self, rhs: Observable) -> Observable {
        Observable::Sub(Box::new(self), Box::new(rhs))
    }
}

impl Mul for Observable {
    type Output = Observable;
    fn mul(self, rhs: Observable) -> Observable {
        Observable::Mul(Box::new(self), Box::new(rhs))
    }
}

impl Div for Observable {
    type Output = Observable;
    fn div(self, rhs: Observable) -> Observable {
        Observable::Div(Box::new(self), Box::new(rhs))
    }
}

impl Neg for Observable {
    type Output = Observable;
    fn neg(self) -> Observable {
        Observable::Neg(Box::new(self))
    }
}

/// Condition : un processus booléen (date atteinte, barrière, comparaison).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Condition {
    /// Constante booléenne.
    Bool(bool),
    /// Vrai à partir de l'instant `t` (en années).
    At(f64),
    /// `lhs >= rhs`.
    Ge(Observable, Observable),
    /// `lhs > rhs`.
    Gt(Observable, Observable),
    /// `lhs <= rhs`.
    Le(Observable, Observable),
    /// `lhs < rhs`.
    Lt(Observable, Observable),
    /// Conjonction.
    And(Box<Condition>, Box<Condition>),
    /// Disjonction.
    Or(Box<Condition>, Box<Condition>),
    /// Négation.
    Not(Box<Condition>),
}

impl Condition {
    /// Conjonction `self && other`.
    pub fn and(self, other: Condition) -> Condition {
        Condition::And(Box::new(self), Box::new(other))
    }

    /// Disjonction `self || other`.
    pub fn or(self, other: Condition) -> Condition {
        Condition::Or(Box::new(self), Box::new(other))
    }
}

impl Not for Condition {
    type Output = Condition;
    fn not(self) -> Condition {
        Condition::Not(Box::new(self))
    }
}

/// Contract : les combinateurs primitifs de l'algèbre.
///
/// Tout produit financier nommé (call, asian, knock-out…) est une *expression*
/// composée de ces primitives — il n'existe aucun cas spécial par produit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Contract {
    /// Contrat nul : aucun flux.
    Zero,
    /// Reçoit une unité de la devise donnée.
    One(Currency),
    /// Inverse les flux du contrat (la contrepartie).
    Give(Box<Contract>),
    /// Détient les deux contrats simultanément.
    And(Box<Contract>, Box<Contract>),
    /// Choix : détient l'un *ou* l'autre (au mieux du détenteur).
    Or(Box<Contract>, Box<Contract>),
    /// Met à l'échelle les flux du contrat par un observable.
    Scale(Observable, Box<Contract>),
    /// Acquiert le contrat dès que la condition devient vraie.
    When(Condition, Box<Contract>),
    /// Peut acquérir le contrat à tout moment où la condition est vraie.
    Anytime(Condition, Box<Contract>),
    /// Détient le contrat jusqu'à ce que la condition devienne vraie.
    Until(Condition, Box<Contract>),
}

impl Contract {
    /// Sérialise le contrat en JSON.
    pub fn to_json(&self) -> Result<String, KontractError> {
        serde_json::to_string(self).map_err(|e| KontractError::Serde(e.to_string()))
    }

    /// Désérialise un contrat depuis du JSON.
    pub fn from_json(s: &str) -> Result<Self, KontractError> {
        serde_json::from_str(s).map_err(|e| KontractError::Serde(e.to_string()))
    }
}

// --- Constructeurs ergonomiques (DSL pur) -------------------------------------
//
// Ces fonctions libres permettent d'écrire les contrats de façon proche de la
// notation mathématique. Elles ne font qu'assembler l'AST.

/// Constante observable.
pub fn konst(x: f64) -> Observable {
    Observable::Const(x)
}

/// Prix spot d'un sous-jacent.
pub fn spot(name: impl Into<String>) -> Observable {
    Observable::Spot(name.into())
}

/// Condition « à l'instant `t` ».
pub fn at(t: f64) -> Condition {
    Condition::At(t)
}

/// Contrat nul.
pub fn zero() -> Contract {
    Contract::Zero
}

/// Une unité de devise.
pub fn one(ccy: impl Into<String>) -> Contract {
    Contract::One(Currency::new(ccy))
}

/// Inverse les flux.
pub fn give(c: Contract) -> Contract {
    Contract::Give(Box::new(c))
}

/// Conjonction de contrats.
pub fn and(a: Contract, b: Contract) -> Contract {
    Contract::And(Box::new(a), Box::new(b))
}

/// Choix entre deux contrats.
pub fn or(a: Contract, b: Contract) -> Contract {
    Contract::Or(Box::new(a), Box::new(b))
}

/// Mise à l'échelle d'un contrat par un observable.
pub fn scale(obs: Observable, c: Contract) -> Contract {
    Contract::Scale(obs, Box::new(c))
}

/// Acquisition à la première vérification de la condition.
pub fn when(cond: Condition, c: Contract) -> Contract {
    Contract::When(cond, Box::new(c))
}

/// Acquisition possible à tout instant où la condition est vraie.
pub fn anytime(cond: Condition, c: Contract) -> Contract {
    Contract::Anytime(cond, Box::new(c))
}

/// Détention jusqu'à ce que la condition devienne vraie.
pub fn until(cond: Condition, c: Contract) -> Contract {
    Contract::Until(cond, Box::new(c))
}
