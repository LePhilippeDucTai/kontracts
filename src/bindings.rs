//! Bindings PyO3 — branchés au jalon J8.
//! Pour l'instant, le module Python expose seulement un marqueur de version.

#![cfg(feature = "python")]

use pyo3::prelude::*;

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    // J8 : enregistrer ici la classe `Contract` et les constructeurs
    // (one, give, scale, when, until, anytime, ...).
    Ok(())
}
