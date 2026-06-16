"""
Calibration de la volatilité GBM — k.fit_gbm_volatility.

Procédure :
  1. Fixer un prix de marché analytique BS à σ_vrai = 0.22.
  2. Calibrer σ via k.fit_gbm_volatility (Gauss-Newton, départ σ_0 = 0.20).
  3. Vérifier |σ_calibré − σ_vrai| < 0.01.

Note pédagogique : l'optimiseur est un Gauss-Newton amorti (pas = −Σνᵢrᵢ/Σνᵢ²
en common random numbers). Le pas s'auto-amortit près de l'optimum, donc il
atteint une cible σ quelconque (et non seulement les multiples du pas) en
quelques itérations. Pour une inversion exacte sur options vanilles, préférer
k.implied_volatility (formule analytique BS, erreur < 1e-7) ; fit_gbm_volatility
sert aux contrats path-dependent (asiatiques, barrières) sans formule fermée.
"""

import math
import kontract as k

# ---------------------------------------------------------------------------
# Référence Black-Scholes (pour comparaison)
# ---------------------------------------------------------------------------

def _norm_cdf(x: float) -> float:
    return 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))


def bs_call(s: float, strike: float, r: float, sigma: float, t: float) -> float:
    d1 = (math.log(s / strike) + (r + 0.5 * sigma ** 2) * t) / (sigma * math.sqrt(t))
    d2 = d1 - sigma * math.sqrt(t)
    return s * _norm_cdf(d1) - strike * math.exp(-r * t) * _norm_cdf(d2)


# ---------------------------------------------------------------------------
# Paramètres
# ---------------------------------------------------------------------------
S0         = 100.0
K          = 100.0
T          = 1.0
R          = 0.05
SIGMA_TRUE = 0.22  # volatilité « de marché »
N_PATHS_MC = 5_000  # chemins pour la calibration (rapide)
SEED       = 42

# ---------------------------------------------------------------------------
# 1. Prix de marché simulé à σ_vrai
# ---------------------------------------------------------------------------
model_true   = k.GBM(s0=S0, sigma=SIGMA_TRUE, r=R, asset="X")
call_contract = k.european_call("X", K, T, k.USD)

# Prix analytique BS comme référence
bs_price = bs_call(S0, K, R, SIGMA_TRUE, T)

print("=" * 60)
print("  CALIBRATION σ GBM — k.fit_gbm_volatility")
print("=" * 60)
print(f"  σ vrai    : {SIGMA_TRUE}")
print(f"  S0={S0}, K={K}, T={T}Y, r={R}")

print(f"\n--- 1. Prix de marché à σ_vrai = {SIGMA_TRUE} ---")
print(f"  Prix BS analytique  : {bs_price:.6f}")

# ---------------------------------------------------------------------------
# 2. Calibration via fit_gbm_volatility
#    Signature : (contract, maturities, market_prices, rate, n_paths)
#    market_prices : liste de tuples (spot, prix_observé)
# ---------------------------------------------------------------------------
# On fournit le prix BS comme prix de marché (référence exacte sans bruit MC)
market_prices = [(S0, bs_price)]

sigma_hat = k.fit_gbm_volatility(
    call_contract,
    [T],
    market_prices,
    R,
    n_paths=N_PATHS_MC,
)

err = abs(sigma_hat - SIGMA_TRUE)

print(f"\n--- 2. Résultat de la calibration ---")
print(f"  σ estimé (fit_gbm)  : {sigma_hat:.6f}")
print(f"  σ vrai              : {SIGMA_TRUE}")
print(f"  |σ_est − σ_vrai|    : {err:.4f}")
print(f"  Tolérance admissible: 0.01")

# Vérification : l'optimiseur Gauss-Newton converge depuis σ_0=0.20 vers σ_vrai
# (le pas s'auto-amortit près de l'optimum → pas d'oscillation, cible quelconque).
assert err < 0.01, (
    f"Calibration hors tolérance : |{sigma_hat:.4f} − {SIGMA_TRUE}| = {err:.4f} ≥ 0.01"
)
print(f"  [OK] Erreur < 0.01")

# ---------------------------------------------------------------------------
# 3. Comparaison des prix reproduced à σ_estimé vs σ_vrai
# ---------------------------------------------------------------------------
model_hat = k.GBM(s0=S0, sigma=sigma_hat, r=R, asset="X")
res_hat   = call_contract.price(model_hat, n_paths=100_000, seed=SEED)
res_true  = call_contract.price(model_true, n_paths=100_000, seed=SEED)

print(f"\n--- 3. Prix reproduit vs prix de marché ---")
print(f"  Prix à σ_vrai  ({SIGMA_TRUE:.2f}) : {res_true.price:.6f}")
print(f"  Prix à σ_est   ({sigma_hat:.2f}) : {res_hat.price:.6f}")
print(f"  Prix BS cible             : {bs_price:.6f}")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 60)
print("  RÉSUMÉ")
print("=" * 60)
print(f"  σ vrai    : {SIGMA_TRUE}")
print(f"  σ calibré : {sigma_hat:.6f}")
print(f"  Erreur    : {err:.4f}  (tolérance 0.01)")
print(f"  Prix BS cible   : {bs_price:.4f}")
print(f"  Prix reproduced : {res_hat.price:.4f}")
print("  Tous les asserts sont verts — script OK")
