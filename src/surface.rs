//! Surfaces de Greeks pour l'analyse de scénarios (jalon J7b).
//!
//! Un trader veut voir δ, γ, ν **en fonction du spot et de la volatilité** :
//! « si le sous-jacent bouge de 5 % et la vol monte d'un point, où est mon
//! hedge ? ». On évalue donc les Greeks sur une grille `(spot × vol)` et on
//! expose le résultat sous forme de matrices, exportables en CSV ou en image
//! grayscale (PGM, sans dépendance externe).

use ndarray::Array2;
use rayon::prelude::*;

use crate::ast::Contract;
use crate::greeks::{greeks_gbm, BumpSizes};
use crate::pricer::McConfig;
use crate::KontractError;

/// Surfaces de prix et de Greeks sur une grille `(spot × vol)`.
///
/// Toutes les matrices sont de forme `[spots.len(), vols.len()]` : la ligne `i`
/// correspond à `spots[i]`, la colonne `j` à `vols[j]`.
#[derive(Debug, Clone)]
pub struct GreekSurface {
    /// Axe des spots.
    pub spots: Vec<f64>,
    /// Axe des volatilités.
    pub vols: Vec<f64>,
    /// Prix.
    pub price: Array2<f64>,
    /// `∂P/∂S`.
    pub delta: Array2<f64>,
    /// `∂²P/∂S²`.
    pub gamma: Array2<f64>,
    /// `∂P/∂σ`.
    pub vega: Array2<f64>,
}

impl GreekSurface {
    /// Exporte une matrice au format CSV (entête `spot\vol`, valeurs tabulées).
    pub fn to_csv(&self, quantity: &Array2<f64>) -> String {
        let header = {
            let vols_str = self
                .vols
                .iter()
                .map(|v| format!(",{v}"))
                .collect::<String>();
            format!("spot\\vol{}\n", vols_str)
        };

        let rows = self
            .spots
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let vals = (0..self.vols.len())
                    .map(|j| format!(",{}", quantity[[i, j]]))
                    .collect::<String>();
                format!("{s}{}\n", vals)
            })
            .collect::<String>();

        format!("{header}{rows}")
    }

    /// Rend une matrice en image grayscale PGM (P2 ASCII), normalisée min→max.
    ///
    /// Aucune dépendance externe : utile pour une visualisation rapide (`feh`,
    /// navigateur, conversion ImageMagick…).
    pub fn to_pgm(&self, quantity: &Array2<f64>) -> String {
        let (rows, cols) = quantity.dim();

        // Calcule min/max avec fold
        let (lo, hi) = quantity.iter().fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(acc_lo, acc_hi), &x| (acc_lo.min(x), acc_hi.max(x)),
        );

        let span = if (hi - lo).abs() < 1e-300 {
            1.0
        } else {
            hi - lo
        };

        let header = format!("P2\n{cols} {rows}\n255\n");
        let lines: String = (0..rows)
            .map(|i| {
                (0..cols)
                    .map(|j| {
                        let g = (((quantity[[i, j]] - lo) / span) * 255.0).round() as i32;
                        format!("{} ", g.clamp(0, 255))
                    })
                    .collect::<String>()
                    + "\n"
            })
            .collect();

        format!("{header}{lines}")
    }
}

/// Calcule les surfaces de prix et de Greeks sur la grille `(spots × vols)`.
///
/// Chaque point de la grille est évalué par [`greeks_gbm`] (bump-and-reprice
/// CRN) ; les points sont parallélisés via `rayon`.
pub fn greek_surface(
    contract: &Contract,
    asset: &str,
    spots: &[f64],
    vols: &[f64],
    cfg: &McConfig,
    bumps: &BumpSizes,
) -> Result<GreekSurface, KontractError> {
    let (ns, nv) = (spots.len(), vols.len());
    if ns == 0 || nv == 0 {
        return Err(KontractError::InconsistentPath(
            "grille de surface vide".into(),
        ));
    }

    // Évaluation parallèle de chaque cellule (i, j).
    let cells = (0..ns * nv)
        .into_par_iter()
        .map(|idx| {
            let (i, j) = (idx / nv, idx % nv);
            greeks_gbm(contract, asset, spots[i], vols[j], cfg, bumps)
        })
        .collect::<Result<Vec<_>, KontractError>>()?;

    let to_arr = |v: Vec<f64>| Array2::from_shape_vec((ns, nv), v).expect("dimensions cohérentes");

    Ok(GreekSurface {
        spots: spots.to_vec(),
        vols: vols.to_vec(),
        price: to_arr(cells.iter().map(|g| g.price).collect()),
        delta: to_arr(cells.iter().map(|g| g.delta).collect()),
        gamma: to_arr(cells.iter().map(|g| g.gamma).collect()),
        vega: to_arr(cells.iter().map(|g| g.vega).collect()),
    })
}
