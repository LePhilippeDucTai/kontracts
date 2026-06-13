//! `kontract` — algèbre des contrats financiers.
//!
//! Couches (cf. CLAUDE.md) :
//!   ast        : DSL pur et sérialisable (combinateurs primitifs)
//!   observable : évaluation des observables sur un path simulé
//!   simulator  : génération de paths Monte-Carlo (GBM au départ)
//!   compiler   : AST -> timeline d'événements / plan de calcul
//!   pricer     : agrégation compositionnelle, discount, Greeks
//!
//! L'implémentation se fait jalon par jalon ; voir ROADMAP.md / PROGRESS.md.
//! Les modules sont déclarés au fur et à mesure qu'ils sont créés.

mod error;
pub use error::KontractError;

pub mod ast; // J1
pub use ast::{Condition, Contract, Currency, Observable};

pub mod observable; // J2
pub use observable::Path;

pub mod simulator; // J3
pub use simulator::Gbm;
// pub mod simulator;    // J3
// pub mod compiler;     // J4
// pub mod pricer;       // J5+

#[cfg(feature = "python")]
mod bindings; // J8

#[cfg(feature = "python")]
use pyo3::prelude::*;

/// Module Python natif. Le contenu réel est branché au jalon J8.
#[cfg(feature = "python")]
#[pymodule]
fn _kontract(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    bindings::register(m)?;
    Ok(())
}
