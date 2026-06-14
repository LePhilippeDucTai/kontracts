//! Bindings PyO3 (jalon J8).
//!
//! Expose l'algèbre et le pricer à Python avec une surface ergonomique :
//!   - `Observable` : arithmétique (`+ - * /`), `.clip(x)`, comparaisons → `Condition` ;
//!   - `Contract`   : `obs * contract` (scale), `contract @ cond` (when),
//!     `.until(cond)`, `.anytime(cond)`, `+` (and), `-` (give) ;
//!   - `GBM` + `Contract.price(...)` / `Contract.greeks(...)`.
//!
//! Exemple Python :
//! ```python
//! from kontract import S, one, at, USD, GBM
//! call = ((S("AAPL") - 100.0).clip(0.0) * one(USD)) @ at(1.0)
//! res = call.price(GBM(s0=100, sigma=0.2, r=0.05), n_paths=200_000)
//! print(res.price, res.std_error)
//! ```

#![cfg(feature = "python")]

use pyo3::class::basic::CompareOp;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;

use crate::ast::{Condition, Contract, Observable};
use crate::greeks::{greeks_gbm, BumpSizes};
use crate::pricer::{price_gbm, McConfig};
use crate::simulator::Gbm;
use crate::KontractError;

/// Convertit une erreur interne en exception Python.
fn to_py_err(e: KontractError) -> PyErr {
    PyValueError::new_err(e.to_string())
}

// =============================================================================
// Observable
// =============================================================================

#[pyclass(name = "Observable")]
#[derive(Clone)]
pub struct PyObservable {
    inner: Observable,
}

#[pymethods]
impl PyObservable {
    /// `max(self, floor)`.
    fn clip(&self, floor: f64) -> PyObservable {
        PyObservable {
            inner: self.inner.clone().clip(floor),
        }
    }

    /// `max(self, other)`.
    fn max(&self, other: &PyObservable) -> PyObservable {
        PyObservable {
            inner: self.inner.clone().max(other.inner.clone()),
        }
    }

    /// `min(self, other)`.
    fn min(&self, other: &PyObservable) -> PyObservable {
        PyObservable {
            inner: self.inner.clone().min(other.inner.clone()),
        }
    }

    fn __add__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObservable> {
        Ok(PyObservable {
            inner: self.inner.clone() + extract_obs(other)?,
        })
    }
    fn __radd__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObservable> {
        Ok(PyObservable {
            inner: extract_obs(other)? + self.inner.clone(),
        })
    }
    fn __sub__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObservable> {
        Ok(PyObservable {
            inner: self.inner.clone() - extract_obs(other)?,
        })
    }
    fn __rsub__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObservable> {
        Ok(PyObservable {
            inner: extract_obs(other)? - self.inner.clone(),
        })
    }
    fn __truediv__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObservable> {
        Ok(PyObservable {
            inner: self.inner.clone() / extract_obs(other)?,
        })
    }
    fn __neg__(&self) -> PyObservable {
        PyObservable {
            inner: -self.inner.clone(),
        }
    }

    /// `*` : soit mise à l'échelle d'un contrat (`obs * contract`), soit produit
    /// d'observables / par un scalaire.
    fn __mul__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<PyObject> {
        if let Ok(c) = other.extract::<PyContract>() {
            return Ok(PyContract {
                inner: Contract::Scale(self.inner.clone(), Box::new(c.inner)),
            }
            .into_py(py));
        }
        Ok(PyObservable {
            inner: self.inner.clone() * extract_obs(other)?,
        }
        .into_py(py))
    }
    fn __rmul__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObservable> {
        Ok(PyObservable {
            inner: extract_obs(other)? * self.inner.clone(),
        })
    }

    /// Comparaisons → `Condition` (barrières/exercice).
    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<PyCondition> {
        let rhs = extract_obs(other)?;
        let lhs = self.inner.clone();
        let cond = match op {
            CompareOp::Ge => Condition::Ge(lhs, rhs),
            CompareOp::Gt => Condition::Gt(lhs, rhs),
            CompareOp::Le => Condition::Le(lhs, rhs),
            CompareOp::Lt => Condition::Lt(lhs, rhs),
            _ => {
                return Err(PyTypeError::new_err(
                    "seuls >=, >, <=, < produisent une condition",
                ))
            }
        };
        Ok(PyCondition { inner: cond })
    }

    fn __repr__(&self) -> String {
        format!("Observable({:?})", self.inner)
    }
}

/// Extrait un `Observable` depuis un float ou un `PyObservable`.
fn extract_obs(any: &Bound<'_, PyAny>) -> PyResult<Observable> {
    if let Ok(o) = any.extract::<PyObservable>() {
        Ok(o.inner)
    } else if let Ok(f) = any.extract::<f64>() {
        Ok(Observable::Const(f))
    } else {
        Err(PyTypeError::new_err("attendu : nombre ou Observable"))
    }
}

// =============================================================================
// Condition
// =============================================================================

#[pyclass(name = "Condition")]
#[derive(Clone)]
pub struct PyCondition {
    inner: Condition,
}

#[pymethods]
impl PyCondition {
    fn __and__(&self, other: &PyCondition) -> PyCondition {
        PyCondition {
            inner: self.inner.clone().and(other.inner.clone()),
        }
    }
    fn __or__(&self, other: &PyCondition) -> PyCondition {
        PyCondition {
            inner: self.inner.clone().or(other.inner.clone()),
        }
    }
    fn __invert__(&self) -> PyCondition {
        PyCondition {
            inner: !self.inner.clone(),
        }
    }
    fn __repr__(&self) -> String {
        format!("Condition({:?})", self.inner)
    }
}

// =============================================================================
// Contract
// =============================================================================

#[pyclass(name = "Contract")]
#[derive(Clone)]
pub struct PyContract {
    inner: Contract,
}

#[pymethods]
impl PyContract {
    /// `contract @ cond` → acquisition à la première activation de `cond`.
    fn __matmul__(&self, cond: &PyCondition) -> PyContract {
        PyContract {
            inner: self.inner.clone().when(cond.inner.clone()),
        }
    }
    /// `+` → détention conjointe (`and`).
    fn __add__(&self, other: &PyContract) -> PyContract {
        PyContract {
            inner: self.inner.clone().and(other.inner.clone()),
        }
    }
    /// `-` unaire → `give` (inversion des flux).
    fn __neg__(&self) -> PyContract {
        PyContract {
            inner: self.inner.clone().give(),
        }
    }
    /// Knock-out jusqu'à activation de `cond`.
    fn until(&self, cond: &PyCondition) -> PyContract {
        PyContract {
            inner: self.inner.clone().until(cond.inner.clone()),
        }
    }
    /// Exercice first-touch dès activation de `cond`.
    fn anytime(&self, cond: &PyCondition) -> PyContract {
        PyContract {
            inner: self.inner.clone().anytime(cond.inner.clone()),
        }
    }
    /// Choix entre deux contrats.
    fn or_(&self, other: &PyContract) -> PyContract {
        PyContract {
            inner: self.inner.clone().or(other.inner.clone()),
        }
    }

    /// Sérialisation JSON.
    fn to_json(&self) -> PyResult<String> {
        self.inner.to_json().map_err(to_py_err)
    }
    /// Désérialisation JSON.
    #[staticmethod]
    fn from_json(s: &str) -> PyResult<PyContract> {
        Ok(PyContract {
            inner: Contract::from_json(s).map_err(to_py_err)?,
        })
    }

    /// Price le contrat sous un modèle GBM.
    #[pyo3(signature = (model, n_paths=100_000, seed=42, steps_per_year=50))]
    fn price(
        &self,
        model: &PyGbm,
        n_paths: usize,
        seed: u64,
        steps_per_year: usize,
    ) -> PyResult<PyPriceResult> {
        let cfg = McConfig {
            n_paths,
            seed,
            steps_per_year,
            rate: model.r,
            variance_reduction: None,
        };
        let res = price_gbm(&self.inner, &model.to_gbm(), &cfg).map_err(to_py_err)?;
        Ok(PyPriceResult {
            price: res.price,
            std_error: res.std_error,
            sample_std: res.sample_std,
            ci95_low: res.ci95_low,
            ci95_high: res.ci95_high,
            n_paths: res.n_paths,
        })
    }

    /// Prix + Greeks (delta, gamma, vega, rho) sous un modèle GBM.
    #[pyo3(signature = (model, n_paths=200_000, seed=42, steps_per_year=50))]
    fn greeks(
        &self,
        model: &PyGbm,
        n_paths: usize,
        seed: u64,
        steps_per_year: usize,
    ) -> PyResult<PyGreeks> {
        let cfg = McConfig {
            n_paths,
            seed,
            steps_per_year,
            rate: model.r,
            variance_reduction: None,
        };
        let g = greeks_gbm(
            &self.inner,
            &model.asset,
            model.s0,
            model.sigma,
            &cfg,
            &BumpSizes::default(),
        )
        .map_err(to_py_err)?;
        Ok(PyGreeks {
            price: g.price,
            delta: g.delta,
            gamma: g.gamma,
            vega: g.vega,
            rho: g.rho,
        })
    }

    fn __repr__(&self) -> String {
        format!("Contract({:?})", self.inner)
    }
}

// =============================================================================
// Modèle GBM
// =============================================================================

#[pyclass(name = "GBM")]
#[derive(Clone)]
pub struct PyGbm {
    #[pyo3(get)]
    asset: String,
    #[pyo3(get)]
    s0: f64,
    #[pyo3(get)]
    sigma: f64,
    #[pyo3(get)]
    r: f64,
}

#[pymethods]
impl PyGbm {
    /// `GBM(s0, sigma, r, asset="ASSET")` — drift risque-neutre = `r`.
    #[new]
    #[pyo3(signature = (s0, sigma, r, asset="ASSET".to_string()))]
    fn new(s0: f64, sigma: f64, r: f64, asset: String) -> Self {
        PyGbm {
            asset,
            s0,
            sigma,
            r,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "GBM(asset={:?}, s0={}, sigma={}, r={})",
            self.asset, self.s0, self.sigma, self.r
        )
    }
}

impl PyGbm {
    fn to_gbm(&self) -> Gbm {
        Gbm::new(&self.asset, self.s0, self.r, self.sigma)
    }
}

// =============================================================================
// Résultats
// =============================================================================

#[pyclass(name = "PriceResult")]
#[derive(Clone)]
pub struct PyPriceResult {
    #[pyo3(get)]
    price: f64,
    #[pyo3(get)]
    std_error: f64,
    #[pyo3(get)]
    sample_std: f64,
    #[pyo3(get)]
    ci95_low: f64,
    #[pyo3(get)]
    ci95_high: f64,
    #[pyo3(get)]
    n_paths: usize,
}

#[pymethods]
impl PyPriceResult {
    fn __repr__(&self) -> String {
        format!(
            "PriceResult(price={:.6}, std_error={:.6}, ci95=[{:.6}, {:.6}], n_paths={})",
            self.price, self.std_error, self.ci95_low, self.ci95_high, self.n_paths
        )
    }
}

#[pyclass(name = "Greeks")]
#[derive(Clone)]
pub struct PyGreeks {
    #[pyo3(get)]
    price: f64,
    #[pyo3(get)]
    delta: f64,
    #[pyo3(get)]
    gamma: f64,
    #[pyo3(get)]
    vega: f64,
    #[pyo3(get)]
    rho: f64,
}

#[pymethods]
impl PyGreeks {
    fn __repr__(&self) -> String {
        format!(
            "Greeks(price={:.6}, delta={:.6}, gamma={:.6}, vega={:.6}, rho={:.6})",
            self.price, self.delta, self.gamma, self.vega, self.rho
        )
    }
}

// =============================================================================
// Constructeurs (fonctions module)
// =============================================================================

#[pyfunction]
fn zero() -> PyContract {
    PyContract {
        inner: Contract::Zero,
    }
}

#[pyfunction]
fn one(ccy: &str) -> PyContract {
    PyContract {
        inner: crate::ast::one(ccy),
    }
}

#[pyfunction]
fn give(c: &PyContract) -> PyContract {
    PyContract {
        inner: c.inner.clone().give(),
    }
}

/// Prix spot d'un sous-jacent (exposé en Python sous les noms `S` et `spot`).
#[pyfunction]
#[pyo3(name = "S")]
fn s_obs(name: &str) -> PyObservable {
    PyObservable {
        inner: crate::ast::spot(name),
    }
}

/// Alias minuscule de `S`.
#[pyfunction]
fn spot(name: &str) -> PyObservable {
    PyObservable {
        inner: crate::ast::spot(name),
    }
}

/// Observable constant.
#[pyfunction]
fn const_(x: f64) -> PyObservable {
    PyObservable {
        inner: Observable::Const(x),
    }
}

/// Condition temporelle « à l'instant t ».
#[pyfunction]
fn at(t: f64) -> PyCondition {
    PyCondition {
        inner: Condition::At(t),
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add("USD", crate::ast::USD)?;
    m.add("EUR", crate::ast::EUR)?;
    m.add("GBP", crate::ast::GBP)?;
    m.add("JPY", crate::ast::JPY)?;

    m.add_class::<PyObservable>()?;
    m.add_class::<PyCondition>()?;
    m.add_class::<PyContract>()?;
    m.add_class::<PyGbm>()?;
    m.add_class::<PyPriceResult>()?;
    m.add_class::<PyGreeks>()?;

    m.add_function(wrap_pyfunction!(zero, m)?)?;
    m.add_function(wrap_pyfunction!(one, m)?)?;
    m.add_function(wrap_pyfunction!(give, m)?)?;
    m.add_function(wrap_pyfunction!(s_obs, m)?)?;
    m.add_function(wrap_pyfunction!(spot, m)?)?;
    m.add_function(wrap_pyfunction!(const_, m)?)?;
    m.add_function(wrap_pyfunction!(at, m)?)?;
    Ok(())
}
