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
    /// Moyenne arithmétique de l'observable sur `[from_year, to_year]`.
    ///
    /// - `from_year = None` → depuis t = 0 (début de la grille).
    /// - `to_year = None` → jusqu'au pas courant d'évaluation.
    ///
    /// Exemple : `Average { obs: spot("X"), from_year: None, to_year: None }`
    /// évalué au pas T donne la moyenne de S_0, …, S_T.
    Average {
        obs: Box<Observable>,
        from_year: Option<f64>,
        to_year: Option<f64>,
    },
    /// Maximum courant de l'observable depuis t = 0 jusqu'au pas courant.
    ///
    /// Payoff lookback à frappe fixe : `RunningMax(spot("X")) - K`.
    RunningMax(Box<Observable>),
    /// Minimum courant de l'observable depuis t = 0 jusqu'au pas courant.
    RunningMin(Box<Observable>),
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

// =============================================================================
// DSL ergonomique (jalon J8a)
//
// Cible : écrire un call à barrière aussi naturellement qu'en notation maths,
// p.ex. `(spot("AAPL") - 150.0).clip(0.0) * one(USD)).when(at(1.0))`.
// L'opérateur Python `@` (→ `when`) est mappé par les bindings PyO3 (J8).
// =============================================================================

/// Codes devises usuels (ergonomie : `one(USD)`).
pub const USD: &str = "USD";
/// Euro.
pub const EUR: &str = "EUR";
/// Livre sterling.
pub const GBP: &str = "GBP";
/// Yen japonais.
pub const JPY: &str = "JPY";

/// Alias court de [`spot`] (proche de la notation `S("AAPL")` du README).
pub fn s(name: impl Into<String>) -> Observable {
    spot(name)
}

/// Moyenne arithmétique de l'observable sur toute la grille jusqu'au pas courant.
///
/// Exemple : `average(spot("AAPL"))` évalué à T donne la moyenne de S_0…S_T.
pub fn average(obs: Observable) -> Observable {
    Observable::Average {
        obs: Box::new(obs),
        from_year: None,
        to_year: None,
    }
}

/// Moyenne arithmétique sur la fenêtre temporelle `[from_year, to_year]`.
pub fn average_over(obs: Observable, from_year: f64, to_year: f64) -> Observable {
    Observable::Average {
        obs: Box::new(obs),
        from_year: Some(from_year),
        to_year: Some(to_year),
    }
}

/// Maximum courant de l'observable depuis t = 0 jusqu'au pas courant.
pub fn running_max(obs: Observable) -> Observable {
    Observable::RunningMax(Box::new(obs))
}

/// Minimum courant de l'observable depuis t = 0 jusqu'au pas courant.
pub fn running_min(obs: Observable) -> Observable {
    Observable::RunningMin(Box::new(obs))
}

impl Observable {
    /// Plancher : `max(self, floor)` — équivaut au `.clip(floor)` du README.
    pub fn clip(self, floor: f64) -> Observable {
        self.max(Observable::Const(floor))
    }
}

// --- Arithmétique observable ⊕ scalaire -------------------------------------

impl Add<f64> for Observable {
    type Output = Observable;
    fn add(self, rhs: f64) -> Observable {
        self + Observable::Const(rhs)
    }
}

impl Sub<f64> for Observable {
    type Output = Observable;
    fn sub(self, rhs: f64) -> Observable {
        self - Observable::Const(rhs)
    }
}

impl Mul<f64> for Observable {
    type Output = Observable;
    fn mul(self, rhs: f64) -> Observable {
        self * Observable::Const(rhs)
    }
}

impl Div<f64> for Observable {
    type Output = Observable;
    fn div(self, rhs: f64) -> Observable {
        self / Observable::Const(rhs)
    }
}

impl Add<Observable> for f64 {
    type Output = Observable;
    fn add(self, rhs: Observable) -> Observable {
        Observable::Const(self) + rhs
    }
}

impl Sub<Observable> for f64 {
    type Output = Observable;
    fn sub(self, rhs: Observable) -> Observable {
        Observable::Const(self) - rhs
    }
}

impl Mul<Observable> for f64 {
    type Output = Observable;
    fn mul(self, rhs: Observable) -> Observable {
        Observable::Const(self) * rhs
    }
}

// --- Mise à l'échelle d'un contrat : `observable * contract` ------------------

impl Mul<Contract> for Observable {
    type Output = Contract;
    fn mul(self, c: Contract) -> Contract {
        Contract::Scale(self, Box::new(c))
    }
}

impl Mul<Contract> for f64 {
    type Output = Contract;
    fn mul(self, c: Contract) -> Contract {
        Contract::Scale(Observable::Const(self), Box::new(c))
    }
}

// --- Méthodes fluides sur les contrats --------------------------------------

impl Contract {
    /// `self` acquis à la première vérification de `cond` (Python : `c @ cond`).
    pub fn when(self, cond: Condition) -> Contract {
        Contract::When(cond, Box::new(self))
    }

    /// `self` détenu jusqu'à activation de `cond` (knock-out).
    pub fn until(self, cond: Condition) -> Contract {
        Contract::Until(cond, Box::new(self))
    }

    /// `self` exerçable dès activation de `cond` (first-touch, cf. J6).
    pub fn anytime(self, cond: Condition) -> Contract {
        Contract::Anytime(cond, Box::new(self))
    }

    /// Détient `self` et `other`.
    pub fn and(self, other: Contract) -> Contract {
        Contract::And(Box::new(self), Box::new(other))
    }

    /// Choix entre `self` et `other`.
    pub fn or(self, other: Contract) -> Contract {
        Contract::Or(Box::new(self), Box::new(other))
    }

    /// Inverse les flux de `self`.
    pub fn give(self) -> Contract {
        Contract::Give(Box::new(self))
    }
}
