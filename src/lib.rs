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

pub mod numerics; // Numerical primitives (centralized)

pub mod ast; // J1
pub use ast::{Condition, Contract, Currency, Observable};

pub mod observable; // J2
pub use observable::Path;

pub mod simulator; // J3, J12, J13, J14
pub use simulator::{
    dupire_from_gbm_calls, heston_from_params, merton_from_params, rough_bergomi_from_params,
    sabr_from_params, DupireSimulator, Gbm, HestonSimulator, MertonJumpSimulator,
    RoughBergomiSimulator, SABRSimulator, Simulator,
};

pub mod compiler; // J4
pub use compiler::{compile, Plan};

pub mod pricer; // J5
pub use pricer::{
    present_value_pub, price_batch_gbm, price_gbm, price_on_paths, McConfig, PriceResult,
};

pub mod variance_reduction; // J15
pub use variance_reduction::VarianceReductionConfig;

pub mod sobol_simulator; // J16
pub use sobol_simulator::{SobolGbm, SobolSimulator};

pub mod lsm; // J17
pub use lsm::{price_american_lsm, LsmConfig};

pub mod mlmc; // J18
pub use mlmc::{
    estimate_variance_at_level, optimal_allocation, price_mlmc, price_mlmc_detailed, MlmcConfig,
    MlmcResult,
};

pub mod market_data; // J21
pub use market_data::{
    build_surface, implied_volatility, load_csv, OptionQuote, VolatilitySurface,
};

pub mod optimizer; // J22
pub use optimizer::{cmaes_minimize, Bounds, CmaesConfig, OptimizeResult};

pub mod calibration; // J21-fast, J22
pub use calibration::{
    calibrate_heston_cmaes, calibrate_merton_cmaes, calibrate_sabr_cmaes, fit_gbm_volatility,
    fit_heston_parameters, CalibrationResult, FastCalibrationConfig,
};

pub mod greeks; // J7
pub use greeks::{greeks_gbm, BumpSizes, Greeks};

pub mod pde; // J19
pub mod pde_2d; // J20
pub mod surface; // J7b
pub use pde::{PdeConfig, PdeSolver};
pub use pde_2d::{Pde2dConfig, Pde2dSolver};
pub use surface::{greek_surface, GreekSurface};

pub mod products; // J9 (catalogue d'expressions DSL)

pub mod backtest; // J23
pub use backtest::{
    backtest_delta_hedge, bs_delta, delta_hedge_error, historical_model_prices, model_vs_market,
    stability, HedgeBacktestReport, PricingErrorReport, StabilityReport,
};

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
