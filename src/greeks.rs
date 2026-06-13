//! Sensibilités (Greeks) par bump-and-reprice (jalon J7).
//!
//! On estime les dérivées du prix par différences finies, en **rejouant les
//! mêmes nombres aléatoires** (common random numbers) à chaque revalorisation :
//! la graine `cfg.seed` et la grille temporelle ne dépendent pas des paramètres
//! bumpés, donc les incréments browniens sont identiques. La variance de la
//! différence s'effondre, rendant même le gamma (différence seconde) stable.
//!
//! Greeks calculés (priorités trader) :
//!   - **delta** `∂P/∂S`        — différence centrée sur le spot ;
//!   - **gamma** `∂²P/∂S²`      — différence seconde sur le spot ;
//!   - **vega**  `∂P/∂σ`        — différence centrée sur la volatilité ;
//!   - **rho**   `∂P/∂r`        — différence centrée sur le taux (drift + discount).

use crate::ast::Contract;
use crate::pricer::{price_gbm, McConfig};
use crate::simulator::Gbm;
use crate::KontractError;

/// Tailles des bumps de différences finies.
#[derive(Debug, Clone)]
pub struct BumpSizes {
    /// Bump absolu du spot (pour delta/gamma).
    pub spot: f64,
    /// Bump absolu de la volatilité (pour vega).
    pub vol: f64,
    /// Bump absolu du taux (pour rho).
    pub rate: f64,
}

impl Default for BumpSizes {
    fn default() -> Self {
        BumpSizes {
            spot: 1e-2,
            vol: 1e-2,
            rate: 1e-4,
        }
    }
}

/// Prix et sensibilités d'un contrat.
#[derive(Debug, Clone, PartialEq)]
pub struct Greeks {
    /// Prix central.
    pub price: f64,
    /// `∂P/∂S`.
    pub delta: f64,
    /// `∂²P/∂S²`.
    pub gamma: f64,
    /// `∂P/∂σ`.
    pub vega: f64,
    /// `∂P/∂r`.
    pub rho: f64,
}

/// Calcule prix + Greeks d'un contrat sous GBM mono-sous-jacent.
///
/// Le drift risque-neutre est pris égal à `cfg.rate` ; bumper le taux bump donc
/// simultanément le drift et l'actualisation (rho cohérent).
pub fn greeks_gbm(
    contract: &Contract,
    asset: &str,
    s0: f64,
    sigma: f64,
    cfg: &McConfig,
    bumps: &BumpSizes,
) -> Result<Greeks, KontractError> {
    // Revalorisation à paramètres (spot, vol, taux), à graine constante (CRN).
    let reprice = |s0: f64, sigma: f64, rate: f64| -> Result<f64, KontractError> {
        let model = Gbm::new(asset, s0, rate, sigma);
        let cfg = McConfig {
            rate,
            ..cfg.clone()
        };
        Ok(price_gbm(contract, &model, &cfg)?.price)
    };

    let r = cfg.rate;
    let (hs, hv, hr) = (bumps.spot, bumps.vol, bumps.rate);

    let price = reprice(s0, sigma, r)?;

    let p_su = reprice(s0 + hs, sigma, r)?;
    let p_sd = reprice(s0 - hs, sigma, r)?;
    let delta = (p_su - p_sd) / (2.0 * hs);
    let gamma = (p_su - 2.0 * price + p_sd) / (hs * hs);

    let vega = (reprice(s0, sigma + hv, r)? - reprice(s0, sigma - hv, r)?) / (2.0 * hv);
    let rho = (reprice(s0, sigma, r + hr)? - reprice(s0, sigma, r - hr)?) / (2.0 * hr);

    Ok(Greeks {
        price,
        delta,
        gamma,
        vega,
        rho,
    })
}
