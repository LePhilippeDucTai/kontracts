//! Bindings PyO3 (jalon J8, étendus au durcissement post-J25).
//!
//! Expose l'algèbre et le moteur à Python :
//!   - `Observable` : arithmétique (`+ - * /`), `.clip(x)`, comparaisons → `Condition` ;
//!   - `Contract`   : `obs * contract` (scale), `contract @ cond` (when),
//!     `.until(cond)`, `.anytime(cond)`, `+` (and), `-` (give) ; `to/from_json` ;
//!     `.price(model, …)` (tout modèle), `.price_american(…)`, `.price_under_rates(…)`,
//!     `.greeks(GBM, …)` ;
//!   - **modèles** : `GBM` (avec dividende `q`), et `Model` via `heston/sabr/merton/
//!     rough_bergomi/sobol_gbm` ; réduction de variance (`antithetic`/`control_variate`) ;
//!   - **taux** (J24) : `vasicek/hull_white` → `RateModel`, `Swaption`, `swaption_mc`,
//!     `vasicek_swaption_analytic` ;
//!   - **produits** (J9) : `european_call`, `straddle`, … ; **FX** (J25) :
//!     `garman_kohlhagen_call/put`, `fx_forward`, `quanto_call` ;
//!   - **calibration** : `implied_volatility`, `fit_gbm_volatility`.
//!
//! Exemple Python :
//! ```python
//! from kontract import S, one, at, USD, GBM, heston, european_call
//! call = ((S("AAPL") - 100.0).clip(0.0) * one(USD)) @ at(1.0)
//! res = call.price(GBM(s0=100, sigma=0.2, r=0.05, asset="AAPL"), n_paths=200_000)
//! res2 = european_call("AAPL", 100, 1.0, USD).price(
//!     heston(spot=100, v0=0.04, kappa=2, theta=0.04, sigma_v=0.3, rho=-0.5, r=0.05,
//!            asset="AAPL"))
//! ```

#![cfg(feature = "python")]

use pyo3::class::basic::CompareOp;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;

use crate::ast::{Condition, Contract, Observable};
use crate::greeks::{greeks_gbm, BumpSizes};
use crate::lsm::{price_american_lsm, LsmConfig};
use crate::pricer::{price_gbm, McConfig};
use crate::rates::{
    price_under_short_rate, swaption_price_mc, HullWhite, ShortRateModel, Swaption, Vasicek,
};
use crate::simulator::{
    Gbm, HestonSimulator, MertonJumpSimulator, RoughBergomiSimulator, SABRSimulator, Simulator,
};
use crate::sobol_simulator::SobolGbm;
use crate::variance_reduction::VarianceReductionConfig;
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

    // --- Méthodes nommées pour le style pipeline --------------------------------
    // Permettent d'écrire : spot("X").sub(100.0).clip(0.0).scale(one("USD"))
    // au lieu de : (spot("X") - 100.0).clip(0.0) * one("USD")

    fn add(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObservable> {
        Ok(PyObservable {
            inner: self.inner.clone() + extract_obs(other)?,
        })
    }

    fn sub(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObservable> {
        Ok(PyObservable {
            inner: self.inner.clone() - extract_obs(other)?,
        })
    }

    fn mul(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObservable> {
        Ok(PyObservable {
            inner: self.inner.clone() * extract_obs(other)?,
        })
    }

    fn div(&self, other: &Bound<'_, PyAny>) -> PyResult<PyObservable> {
        Ok(PyObservable {
            inner: self.inner.clone() / extract_obs(other)?,
        })
    }

    /// Mise à l'échelle du contrat — `self.scale(contract)` ≡ `self * contract`.
    fn scale(&self, contract: &PyContract) -> PyContract {
        PyContract {
            inner: self.inner.clone().scale(contract.inner.clone()),
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

/// Exécute `f` avec le simulateur sous-jacent (`GBM` ou `Model`) et son taux
/// d'actualisation. Évite de cloner un objet-trait (non-`Clone`) : on emprunte.
fn with_simulator<T>(
    model: &Bound<'_, PyAny>,
    f: impl FnOnce(&dyn Simulator, f64) -> PyResult<T>,
) -> PyResult<T> {
    if let Ok(g) = model.extract::<PyGbm>() {
        f(&g.to_gbm(), g.r)
    } else if let Ok(m) = model.extract::<PyRef<PyModel>>() {
        f(m.inner.as_ref(), m.rate)
    } else {
        Err(PyTypeError::new_err("attendu : modèle GBM ou Model"))
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

    /// Price le contrat sous n'importe quel modèle (`GBM` ou `Model`).
    ///
    /// `antithetic`/`control_variate` activent la réduction de variance (J15) ;
    /// sans effet pour les modèles qui ne la supportent pas.
    #[pyo3(signature = (model, n_paths=100_000, seed=42, steps_per_year=50, antithetic=false, control_variate=false))]
    fn price(
        &self,
        model: &Bound<'_, PyAny>,
        n_paths: usize,
        seed: u64,
        steps_per_year: usize,
        antithetic: bool,
        control_variate: bool,
    ) -> PyResult<PyPriceResult> {
        let vr = if antithetic || control_variate {
            Some(VarianceReductionConfig {
                use_antithetic: antithetic,
                use_control_variate: control_variate,
            })
        } else {
            None
        };
        with_simulator(model, |sim, rate| {
            let cfg = McConfig {
                n_paths,
                seed,
                steps_per_year,
                rate,
                variance_reduction: vr,
            };
            let res = price_gbm(&self.inner, sim, &cfg).map_err(to_py_err)?;
            Ok(PyPriceResult::from(res))
        })
    }

    /// Price une option **américaine** par Longstaff-Schwartz : `self` est le
    /// payoff exercé (p.ex. `(K - S).clip(0) * one(USD)`), exercé aux dates
    /// `exercise_dates`. Mode d'exécution du pricer — l'AST reste inchangé.
    #[pyo3(signature = (model, exercise_dates, n_paths=100_000, seed=42, n_basis=3))]
    fn price_american(
        &self,
        model: &Bound<'_, PyAny>,
        exercise_dates: Vec<f64>,
        n_paths: usize,
        seed: u64,
        n_basis: usize,
    ) -> PyResult<PyPriceResult> {
        with_simulator(model, |sim, rate| {
            let cfg = McConfig {
                n_paths,
                seed,
                steps_per_year: 50,
                rate,
                variance_reduction: None,
            };
            let res = price_american_lsm(
                &self.inner,
                &exercise_dates,
                sim,
                &cfg,
                &LsmConfig { n_basis },
            )
            .map_err(to_py_err)?;
            Ok(PyPriceResult::from(res))
        })
    }

    /// Price le contrat en **actualisation stochastique** sous un modèle de taux
    /// court (`Vasicek`/`HullWhite`) — lève l'actualisation déterministe (J24).
    #[pyo3(signature = (rate_model, n_paths=100_000, seed=42, steps_per_year=50))]
    fn price_under_rates(
        &self,
        rate_model: &PyRateModel,
        n_paths: usize,
        seed: u64,
        steps_per_year: usize,
    ) -> PyResult<PyPriceResult> {
        let cfg = McConfig {
            n_paths,
            seed,
            steps_per_year,
            rate: 0.0,
            variance_reduction: None,
        };
        let res = price_under_short_rate(&self.inner, rate_model.inner.as_ref(), &cfg)
            .map_err(to_py_err)?;
        Ok(PyPriceResult::from(res))
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
    /// Rendement de dividende / portage `q` (drift risque-neutre = `r − q`).
    #[pyo3(get)]
    q: f64,
}

#[pymethods]
impl PyGbm {
    /// `GBM(s0, sigma, r, q=0.0, asset="ASSET")` — drift risque-neutre = `r − q`.
    #[new]
    #[pyo3(signature = (s0, sigma, r, q=0.0, asset="ASSET".to_string()))]
    fn new(s0: f64, sigma: f64, r: f64, q: f64, asset: String) -> Self {
        PyGbm {
            asset,
            s0,
            sigma,
            r,
            q,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "GBM(asset={:?}, s0={}, sigma={}, r={}, q={})",
            self.asset, self.s0, self.sigma, self.r, self.q
        )
    }
}

impl PyGbm {
    fn to_gbm(&self) -> Gbm {
        // Drift risque-neutre avec portage : μ = r − q.
        Gbm::new(&self.asset, self.s0, self.r - self.q, self.sigma)
    }
}

// =============================================================================
// Modèle générique (Heston, SABR, Merton, Rough Bergomi, Sobol GBM)
// =============================================================================

/// Wrapper opaque d'un simulateur quelconque, partagé avec le pricer via le trait
/// [`Simulator`]. Construit par les fabriques `heston()`, `sabr()`, etc.
#[pyclass(name = "Model")]
pub struct PyModel {
    inner: Box<dyn Simulator>,
    /// Taux d'actualisation déterministe (= taux risque-neutre du modèle).
    rate: f64,
    /// Étiquette lisible.
    label: String,
}

#[pymethods]
impl PyModel {
    fn __repr__(&self) -> String {
        format!("Model({}, rate={})", self.label, self.rate)
    }
}

/// Modèle de Heston (vol stochastique).
#[pyfunction]
#[pyo3(signature = (spot, v0, kappa, theta, sigma_v, rho, r, asset="ASSET".to_string()))]
#[allow(clippy::too_many_arguments)]
fn heston(
    spot: f64,
    v0: f64,
    kappa: f64,
    theta: f64,
    sigma_v: f64,
    rho: f64,
    r: f64,
    asset: String,
) -> PyModel {
    PyModel {
        inner: Box::new(HestonSimulator::new(
            &asset, spot, v0, kappa, theta, sigma_v, rho, r,
        )),
        rate: r,
        label: "Heston".into(),
    }
}

/// Modèle SABR (CEV stochastique).
#[pyfunction]
#[pyo3(signature = (spot, alpha, beta, nu, rho, r, asset="ASSET".to_string()))]
fn sabr(spot: f64, alpha: f64, beta: f64, nu: f64, rho: f64, r: f64, asset: String) -> PyModel {
    PyModel {
        inner: Box::new(SABRSimulator::new(&asset, spot, alpha, beta, nu, rho, r)),
        rate: r,
        label: "SABR".into(),
    }
}

/// Modèle de Merton (diffusion + sauts de Poisson composés).
#[pyfunction]
#[pyo3(signature = (spot, r, sigma, lambda, mu_j, sigma_j, asset="ASSET".to_string()))]
#[allow(clippy::too_many_arguments)]
fn merton(
    spot: f64,
    r: f64,
    sigma: f64,
    lambda: f64,
    mu_j: f64,
    sigma_j: f64,
    asset: String,
) -> PyModel {
    PyModel {
        inner: Box::new(MertonJumpSimulator::new(
            &asset, spot, r, sigma, lambda, mu_j, sigma_j,
        )),
        rate: r,
        label: "Merton".into(),
    }
}

/// Modèle Rough Bergomi (volatilité rugueuse, fBm).
#[pyfunction]
#[pyo3(signature = (spot, v0, xi, h, rho, r, asset="ASSET".to_string()))]
fn rough_bergomi(spot: f64, v0: f64, xi: f64, h: f64, rho: f64, r: f64, asset: String) -> PyModel {
    PyModel {
        inner: Box::new(RoughBergomiSimulator::new(&asset, spot, v0, xi, h, rho, r)),
        rate: r,
        label: "RoughBergomi".into(),
    }
}

/// GBM en quasi-Monte-Carlo (séquence de Sobol, convergence O(1/N)).
#[pyfunction]
#[pyo3(signature = (spot, sigma, r, q=0.0, asset="ASSET".to_string()))]
fn sobol_gbm(spot: f64, sigma: f64, r: f64, q: f64, asset: String) -> PyModel {
    PyModel {
        inner: Box::new(SobolGbm::new(&asset, spot, r - q, sigma)),
        rate: r,
        label: "SobolGBM".into(),
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

impl From<crate::pricer::PriceResult> for PyPriceResult {
    fn from(r: crate::pricer::PriceResult) -> Self {
        PyPriceResult {
            price: r.price,
            std_error: r.std_error,
            sample_std: r.sample_std,
            ci95_low: r.ci95_low,
            ci95_high: r.ci95_high,
            n_paths: r.n_paths,
        }
    }
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

// =============================================================================
// Taux courts stochastiques (J24)
// =============================================================================

/// Modèle de taux court (`Vasicek` ou `HullWhite`), opaque.
#[pyclass(name = "RateModel")]
pub struct PyRateModel {
    inner: Box<dyn ShortRateModel>,
    label: String,
}

#[pymethods]
impl PyRateModel {
    /// Prix analytique du zéro-coupon `P(t, T)` sachant `r(t)`.
    fn zero_bond(&self, t: f64, big_t: f64, r_t: f64) -> f64 {
        self.inner.zero_bond(t, big_t, r_t)
    }
    /// Prix initial du zéro-coupon `P(0, T)`.
    fn discount_bond0(&self, big_t: f64) -> f64 {
        self.inner.discount_bond0(big_t)
    }
    fn __repr__(&self) -> String {
        format!("RateModel({})", self.label)
    }
}

/// Modèle de Vasicek `dr = a(b − r)dt + σ dW`.
#[pyfunction]
fn vasicek(r0: f64, a: f64, b: f64, sigma: f64) -> PyRateModel {
    PyRateModel {
        inner: Box::new(Vasicek::new(r0, a, b, sigma)),
        label: "Vasicek".into(),
    }
}

/// Modèle de Hull-White sur courbe plate `e^{−r₀T}`.
#[pyfunction]
fn hull_white(r0: f64, a: f64, sigma: f64) -> PyRateModel {
    PyRateModel {
        inner: Box::new(HullWhite::new(r0, a, sigma)),
        label: "HullWhite".into(),
    }
}

/// Swaption européenne sur swap à jambe fixe régulière.
#[pyclass(name = "Swaption")]
#[derive(Clone)]
pub struct PySwaption {
    inner: Swaption,
}

#[pymethods]
impl PySwaption {
    /// Swaption standard : `n` paiements espacés de `tenor` à partir de `expiry`.
    #[staticmethod]
    #[pyo3(signature = (expiry, tenor, n, fixed_rate, is_payer=true))]
    fn level(expiry: f64, tenor: f64, n: usize, fixed_rate: f64, is_payer: bool) -> PySwaption {
        PySwaption {
            inner: Swaption::level(expiry, tenor, n, fixed_rate, is_payer),
        }
    }
}

/// Prix Monte-Carlo d'une swaption sous un modèle de taux court.
#[pyfunction]
#[pyo3(signature = (rate_model, swaption, n_paths=200_000, seed=42, steps=100))]
fn swaption_mc(
    rate_model: &PyRateModel,
    swaption: &PySwaption,
    n_paths: usize,
    seed: u64,
    steps: usize,
) -> PyResult<PyPriceResult> {
    let cfg = McConfig {
        n_paths,
        seed,
        steps_per_year: steps,
        rate: 0.0,
        variance_reduction: None,
    };
    let res = swaption_price_mc(rate_model.inner.as_ref(), &swaption.inner, &cfg, steps)
        .map_err(to_py_err)?;
    Ok(PyPriceResult::from(res))
}

/// Prix analytique (Jamshidian) d'une swaption sous Vasicek.
#[pyfunction]
fn vasicek_swaption_analytic(
    r0: f64,
    a: f64,
    b: f64,
    sigma: f64,
    swaption: &PySwaption,
) -> PyResult<f64> {
    Vasicek::new(r0, a, b, sigma)
        .swaption_analytic(&swaption.inner)
        .map_err(to_py_err)
}

// =============================================================================
// Catalogue de produits (J9) — expressions DSL prêtes à l'emploi
// =============================================================================

macro_rules! product_fn {
    ($name:ident, $($arg:ident : $ty:ty),+) => {
        #[pyfunction]
        fn $name($($arg : $ty),+) -> PyContract {
            PyContract { inner: crate::products::$name($($arg),+) }
        }
    };
}

product_fn!(zero_coupon_bond, ccy: &str, t: f64);
product_fn!(european_call, asset: &str, k: f64, t: f64, ccy: &str);
product_fn!(european_put, asset: &str, k: f64, t: f64, ccy: &str);
product_fn!(forward, asset: &str, k: f64, t: f64, ccy: &str);
product_fn!(straddle, asset: &str, k: f64, t: f64, ccy: &str);
product_fn!(bull_call_spread, asset: &str, k_low: f64, k_high: f64, t: f64, ccy: &str);
product_fn!(cash_or_nothing_call, asset: &str, k: f64, payout: f64, t: f64, ccy: &str);
product_fn!(up_and_out_call, asset: &str, k: f64, barrier: f64, t: f64, ccy: &str);
product_fn!(down_and_out_call, asset: &str, k: f64, barrier: f64, t: f64, ccy: &str);

// =============================================================================
// FX (J25) — références analytiques
// =============================================================================

/// Call de change Garman-Kohlhagen.
#[pyfunction]
fn garman_kohlhagen_call(x0: f64, k: f64, t: f64, r_d: f64, r_f: f64, sigma: f64) -> f64 {
    crate::fx::garman_kohlhagen_call(x0, k, t, r_d, r_f, sigma)
}
/// Put de change Garman-Kohlhagen.
#[pyfunction]
fn garman_kohlhagen_put(x0: f64, k: f64, t: f64, r_d: f64, r_f: f64, sigma: f64) -> f64 {
    crate::fx::garman_kohlhagen_put(x0, k, t, r_d, r_f, sigma)
}
/// Taux de change à terme (parité des taux couverte).
#[pyfunction]
fn fx_forward(x0: f64, t: f64, r_d: f64, r_f: f64) -> f64 {
    crate::fx::fx_forward(x0, t, r_d, r_f)
}
/// Quanto call (corrélation spot/FX via ajustement de drift).
#[pyfunction]
#[allow(clippy::too_many_arguments)]
fn quanto_call(
    s0: f64,
    k: f64,
    t: f64,
    r_d: f64,
    r_f: f64,
    q_s: f64,
    sigma_s: f64,
    sigma_x: f64,
    rho: f64,
) -> f64 {
    crate::fx::quanto_call(s0, k, t, r_d, r_f, q_s, sigma_s, sigma_x, rho)
}

// =============================================================================
// Calibration / données de marché
// =============================================================================

/// Volatilité implicite Black-Scholes (inversion par bissection).
#[pyfunction]
fn implied_volatility(
    call_price: f64,
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    dividend_yield: f64,
) -> PyResult<f64> {
    crate::market_data::implied_volatility(call_price, spot, strike, maturity, rate, dividend_yield)
        .map_err(to_py_err)
}

/// Calibre la volatilité GBM à des prix de marché `[(spot, prix)]` pour un
/// contrat donné. Renvoie la volatilité ajustée.
#[pyfunction]
#[pyo3(signature = (contract, maturities, market_prices, rate, n_paths=2000))]
fn fit_gbm_volatility(
    contract: &PyContract,
    maturities: Vec<f64>,
    market_prices: Vec<(f64, f64)>,
    rate: f64,
    n_paths: usize,
) -> PyResult<f64> {
    let cfg = crate::calibration::FastCalibrationConfig {
        n_paths,
        ..Default::default()
    };
    let res = crate::calibration::fit_gbm_volatility(
        &contract.inner,
        &maturities,
        &market_prices,
        rate,
        &cfg,
    )
    .map_err(to_py_err)?;
    Ok(res.parameters[0])
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
    m.add_class::<PyModel>()?;
    m.add_class::<PyRateModel>()?;
    m.add_class::<PySwaption>()?;
    m.add_class::<PyPriceResult>()?;
    m.add_class::<PyGreeks>()?;

    // DSL de base
    m.add_function(wrap_pyfunction!(zero, m)?)?;
    m.add_function(wrap_pyfunction!(one, m)?)?;
    m.add_function(wrap_pyfunction!(give, m)?)?;
    m.add_function(wrap_pyfunction!(s_obs, m)?)?;
    m.add_function(wrap_pyfunction!(spot, m)?)?;
    m.add_function(wrap_pyfunction!(const_, m)?)?;
    m.add_function(wrap_pyfunction!(at, m)?)?;

    // Modèles (J12–J16)
    m.add_function(wrap_pyfunction!(heston, m)?)?;
    m.add_function(wrap_pyfunction!(sabr, m)?)?;
    m.add_function(wrap_pyfunction!(merton, m)?)?;
    m.add_function(wrap_pyfunction!(rough_bergomi, m)?)?;
    m.add_function(wrap_pyfunction!(sobol_gbm, m)?)?;

    // Taux (J24)
    m.add_function(wrap_pyfunction!(vasicek, m)?)?;
    m.add_function(wrap_pyfunction!(hull_white, m)?)?;
    m.add_function(wrap_pyfunction!(swaption_mc, m)?)?;
    m.add_function(wrap_pyfunction!(vasicek_swaption_analytic, m)?)?;

    // Produits (J9)
    m.add_function(wrap_pyfunction!(zero_coupon_bond, m)?)?;
    m.add_function(wrap_pyfunction!(european_call, m)?)?;
    m.add_function(wrap_pyfunction!(european_put, m)?)?;
    m.add_function(wrap_pyfunction!(forward, m)?)?;
    m.add_function(wrap_pyfunction!(straddle, m)?)?;
    m.add_function(wrap_pyfunction!(bull_call_spread, m)?)?;
    m.add_function(wrap_pyfunction!(cash_or_nothing_call, m)?)?;
    m.add_function(wrap_pyfunction!(up_and_out_call, m)?)?;
    m.add_function(wrap_pyfunction!(down_and_out_call, m)?)?;

    // FX (J25)
    m.add_function(wrap_pyfunction!(garman_kohlhagen_call, m)?)?;
    m.add_function(wrap_pyfunction!(garman_kohlhagen_put, m)?)?;
    m.add_function(wrap_pyfunction!(fx_forward, m)?)?;
    m.add_function(wrap_pyfunction!(quanto_call, m)?)?;

    // Calibration / données de marché
    m.add_function(wrap_pyfunction!(implied_volatility, m)?)?;
    m.add_function(wrap_pyfunction!(fit_gbm_volatility, m)?)?;
    Ok(())
}
