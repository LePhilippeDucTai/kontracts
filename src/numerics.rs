//! Fonctions numériques centralisées pour tout le crate.
//!
//! Module source unique de vérité pour les primitives mathématiques :
//! - CDF/PDF de la normale
//! - Black-Scholes (call/put)
//! - Résolveurs linéaires (Thomas tridiagonal, Cholesky, Gauss)
//!
//! **Ordre canonique Black-Scholes** : `(s, k, t, r, sigma)`
//! garantit la cohérence numérique sur tous les sites d'appel.

use std::f64::consts::{PI, SQRT_2};

use crate::KontractError;

// ============================================================================
// Approximation erfc (Abramowitz-Stegun 7.1.26)
// ============================================================================

/// Approximation rationnelle de la fonction d'erreur complémentaire.
/// Précision ~1e-7 ; utilisée pour calculer `N(x)` sans dépendance externe.
#[inline]
fn erfc_as(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0_f64 } else { 1.0_f64 };
    let x_abs = x.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * x_abs);
    let poly = ((((1.061_405_429 * t - 1.453_152_027) * t) + 1.421_413_741) * t - 0.284_496_736)
        * t
        + 0.254_829_592;
    let erf_abs = 1.0 - poly * t * (-x_abs * x_abs).exp();
    0.5 * (1.0 + sign * erf_abs)
}

// ============================================================================
// CDF et PDF de la loi normale
// ============================================================================

/// CDF de la loi normale standard N(x).
#[inline]
pub fn norm_cdf(x: f64) -> f64 {
    erfc_as(x / SQRT_2)
}

/// PDF de la loi normale standard : (1/√(2π)) exp(-x²/2).
#[inline]
pub fn norm_pdf(x: f64) -> f64 {
    (-0.5 * x * x).exp() / (2.0 * PI).sqrt()
}

// ============================================================================
// Black-Scholes analytique
// ============================================================================

/// Prix Black-Scholes d'un call européen vanille.
///
/// # Paramètres
/// - `s`     : spot initial
/// - `k`     : strike
/// - `t`     : maturité (années)
/// - `r`     : taux sans risque (déterministe)
/// - `sigma` : volatilité implicite
///
/// **Ordre canonical** : `(s, k, t, r, sigma)` — celui de `variance_reduction.rs:77`.
pub fn black_scholes_call(s: f64, k: f64, t: f64, r: f64, sigma: f64) -> f64 {
    if t <= 0.0 || sigma <= 0.0 {
        return (s - k * (-r * t).exp()).max(0.0);
    }
    let vol_sqrt_t = sigma * t.sqrt();
    let d1 = ((s / k).ln() + (r + 0.5 * sigma * sigma) * t) / vol_sqrt_t;
    let d2 = d1 - vol_sqrt_t;
    s * norm_cdf(d1) - k * (-r * t).exp() * norm_cdf(d2)
}

/// Prix Black-Scholes d'un put européen vanille.
///
/// # Paramètres
/// - `s`     : spot initial
/// - `k`     : strike
/// - `t`     : maturité (années)
/// - `r`     : taux sans risque (déterministe)
/// - `sigma` : volatilité implicite
///
/// **Ordre canonical** : `(s, k, t, r, sigma)`.
pub fn black_scholes_put(s: f64, k: f64, t: f64, r: f64, sigma: f64) -> f64 {
    if t <= 0.0 || sigma <= 0.0 {
        return (k * (-r * t).exp() - s).max(0.0);
    }
    let vol_sqrt_t = sigma * t.sqrt();
    let d1 = ((s / k).ln() + (r + 0.5 * sigma * sigma) * t) / vol_sqrt_t;
    let d2 = d1 - vol_sqrt_t;
    k * (-r * t).exp() * norm_cdf(-d2) - s * norm_cdf(-d1)
}

// ============================================================================
// Solveurs linéaires
// ============================================================================

/// Résout un système tridiagonal `A x = rhs` via l'algorithme de Thomas.
///
/// # Paramètres
/// - `a` : sous-diagonale (indices 1..n)
/// - `b` : diagonale principale (indices 0..n)
/// - `c` : sur-diagonale (indices 0..n-1)
/// - `rhs` : second membre
///
/// # Paniques
/// Retourne une erreur si la matrice est singulière.
pub fn thomas(a: &[f64], b: &[f64], c: &[f64], rhs: &[f64]) -> Result<Vec<f64>, KontractError> {
    let n = rhs.len();
    let mut x = vec![0.0; n];

    if n <= 2 {
        x[0] = rhs[0];
        if n == 2 {
            x[1] = rhs[1];
        }
        return Ok(x);
    }

    x[0] = rhs[0];
    x[n - 1] = rhs[n - 1];

    let mut c_mod = vec![0.0; n];
    let mut d_mod = vec![0.0; n];

    c_mod[0] = c[0] / b[0];
    d_mod[0] = rhs[0] / b[0];

    // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
    for i in 1..n {
        let denom = b[i] - a[i] * c_mod[i - 1];
        if denom.abs() < 1e-15 {
            return Err(KontractError::MalformedContract(
                "Singular matrix in Thomas solver".to_string(),
            ));
        }
        if i < n - 1 {
            c_mod[i] = c[i] / denom;
        }
        d_mod[i] = (rhs[i] - a[i] * d_mod[i - 1]) / denom;
    }

    x[n - 1] = d_mod[n - 1];
    // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
    for i in (0..n - 1).rev() {
        x[i] = d_mod[i] - c_mod[i] * x[i + 1];
    }

    Ok(x)
}

/// Décomposition de Cholesky en matrice triangulaire inférieure.
///
/// Implémentation dense classique en `O(n³)`. Pour rester robuste face aux
/// covariances fBm légèrement mal conditionnées (H petit, n grand), les pivots
/// négatifs par erreur d'arrondi sont planchés à 0 (la matrice est alors traitée
/// comme semi-définie positive plutôt que définie positive). Aucune dépendance
/// LAPACK n'est requise.
pub fn cholesky_lower(a: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let n = a.len();
    let mut l = vec![vec![0.0f64; n]; n];
    // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
    for i in 0..n {
        for j in 0..=i {
            let mut sum = a[i][j];
            // Produit scalaire des lignes i et j sur [0, j) ; l'indexation par k
            // est nécessaire (les deux lignes de `l` sont empruntées en lecture).
            // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
            #[allow(clippy::needless_range_loop)]
            for k in 0..j {
                sum -= l[i][k] * l[j][k];
            }
            if i == j {
                // Pivot diagonal : plancher à 0 pour absorber le bruit d'arrondi
                // (covariance fBm SPD en théorie, semi-définie en pratique).
                l[i][j] = sum.max(0.0).sqrt();
            } else {
                let pivot = l[j][j];
                l[i][j] = if pivot > 0.0 { sum / pivot } else { 0.0 };
            }
        }
    }
    l
}

/// Résout `A x = b` par élimination de Gauss avec pivot partiel.
///
/// Renvoie un vecteur nul si le système est singulier (la régression dégénère
/// alors en continuation pure, comportement sûr).
#[allow(clippy::needless_range_loop)] // élimination de Gauss : indices ligne/colonne
pub fn solve_linear(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Result<Vec<f64>, KontractError> {
    let n = b.len();
    // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
    for col in 0..n {
        // Pivot partiel : ligne au plus grand |a[row][col]|.
        let mut pivot = col;
        let mut best = a[col][col].abs();
        // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
        for row in (col + 1)..n {
            let v = a[row][col].abs();
            if v > best {
                best = v;
                pivot = row;
            }
        }
        if best < 1e-12 {
            // Système (quasi) singulier : régression non identifiable → coeffs nuls.
            return Ok(vec![0.0; n]);
        }
        a.swap(col, pivot);
        b.swap(col, pivot);

        // Élimination.
        // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
        for row in (col + 1)..n {
            let factor = a[row][col] / a[col][col];
            // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
            for c in col..n {
                a[row][c] -= factor * a[col][c];
            }
            b[row] -= factor * b[col];
        }
    }

    // Substitution arrière.
    let mut x = vec![0.0f64; n];
    // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
    for row in (0..n).rev() {
        let mut sum = b[row];
        // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
        for c in (row + 1)..n {
            sum -= a[row][c] * x[c];
        }
        x[row] = sum / a[row][row];
    }
    Ok(x)
}

// ============================================================================
// Décomposition spectrale (Jacobi symétrique) — J22
// ============================================================================

/// Diagonalise une matrice **symétrique** `a` (n×n) par rotations de Jacobi.
///
/// Renvoie `(valeurs_propres, vecteurs_propres)` où `vecteurs_propres[k]` est le
/// k-ième vecteur propre (colonne `k` de la matrice de rotation accumulée `V`),
/// associé à `valeurs_propres[k]`. La reconstruction vaut `a = V · diag(λ) · Vᵀ`.
///
/// La méthode de Jacobi est inconditionnellement convergente pour les matrices
/// symétriques et précise pour les **petites dimensions** (n ≤ 6, le cas des
/// covariances CMA-ES : Heston 5 paramètres, SABR 4, Merton 5). Coût `O(n³)` par
/// balayage, quelques balayages suffisent (convergence quadratique en fin de
/// course). Aucune dépendance LAPACK.
#[allow(clippy::needless_range_loop)] // rotations de Jacobi : indexation matricielle volontaire
pub fn jacobi_eigen(a: &[Vec<f64>]) -> (Vec<f64>, Vec<Vec<f64>>) {
    let n = a.len();
    // Copie de travail (la matrice est progressivement diagonalisée).
    let mut m = a.to_vec();
    // Matrice des vecteurs propres, initialisée à l'identité.
    let mut v: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
        .collect();

    // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
    // Balayages de Jacobi : annulation itérative des éléments hors-diagonale par
    // rotations de Givens, avec accumulation des vecteurs propres dans `v`.
    // Échelle de la matrice (norme de Frobenius) pour un seuil d'arrêt **relatif** :
    // on stoppe quand la masse hors-diagonale est négligeable devant l'échelle, ce
    // qui reste robuste pour des covariances anisotropes (mal conditionnées).
    let frob: f64 = (0..n)
        .flat_map(|i| (0..n).map(move |j| (i, j)))
        .map(|(i, j)| m[i][j] * m[i][j])
        .sum::<f64>()
        .sqrt();
    let off_tol = (1e-14 * frob).max(1e-300);
    for _sweep in 0..100 {
        // Magnitude hors-diagonale (critère d'arrêt relatif à l'échelle).
        let off: f64 = (0..n)
            .flat_map(|p| ((p + 1)..n).map(move |q| (p, q)))
            .map(|(p, q)| m[p][q] * m[p][q])
            .sum::<f64>()
            .sqrt();
        if off < off_tol {
            break;
        }

        for p in 0..n {
            for q in (p + 1)..n {
                let apq = m[p][q];
                // Saute les éléments négligeables devant l'échelle de la diagonale.
                if apq.abs() <= 1e-300 * (m[p][p].abs() + m[q][q].abs() + 1.0) {
                    continue;
                }
                // Angle de rotation annulant m[p][q] (formulation stable de Rutishauser).
                let theta = (m[q][q] - m[p][p]) / (2.0 * apq);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;

                // Mise à jour des diagonales et annulation de l'élément (p, q).
                let app = m[p][p];
                let aqq = m[q][q];
                m[p][p] = app - t * apq;
                m[q][q] = aqq + t * apq;
                m[p][q] = 0.0;
                m[q][p] = 0.0;

                // Rotation des autres éléments des lignes/colonnes p et q.
                for i in 0..n {
                    if i != p && i != q {
                        let mip = m[i][p];
                        let miq = m[i][q];
                        m[i][p] = c * mip - s * miq;
                        m[p][i] = m[i][p];
                        m[i][q] = s * mip + c * miq;
                        m[q][i] = m[i][q];
                    }
                }
                // Accumulation des vecteurs propres.
                for i in 0..n {
                    let vip = v[i][p];
                    let viq = v[i][q];
                    v[i][p] = c * vip - s * viq;
                    v[i][q] = s * vip + c * viq;
                }
            }
        }
    }

    let eigenvalues: Vec<f64> = (0..n).map(|i| m[i][i]).collect();
    // Vecteurs propres en colonnes de `v` → renvoyés indexés par k (colonne k).
    let eigenvectors: Vec<Vec<f64>> = (0..n).map(|k| (0..n).map(|i| v[i][k]).collect()).collect();
    (eigenvalues, eigenvectors)
}
