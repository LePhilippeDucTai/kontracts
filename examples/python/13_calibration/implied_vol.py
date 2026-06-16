"""
Volatilité implicite — aller-retour Black-Scholes ↔ k.implied_volatility.

Procédure :
  1. Calculer le prix BS analytique d'un call ATM à σ=0.25.
  2. Retrouver σ_implicite via k.implied_volatility.
  3. Vérifier |σ_implicite − 0.25| < 1e-3.

La formule BS locale sert de référence ; la vol implicite est extraite par
inversion numérique (Brent / Newton dans kontract).
"""

import math
import kontract as k

# ---------------------------------------------------------------------------
# Référence Black-Scholes
# ---------------------------------------------------------------------------

def _norm_cdf(x: float) -> float:
    """CDF gaussienne standard via math.erf."""
    return 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))


def bs_call(s: float, strike: float, r: float, sigma: float, t: float) -> float:
    """Prix BS d'un call européen (sans dividende)."""
    d1 = (math.log(s / strike) + (r + 0.5 * sigma ** 2) * t) / (sigma * math.sqrt(t))
    d2 = d1 - sigma * math.sqrt(t)
    return s * _norm_cdf(d1) - strike * math.exp(-r * t) * _norm_cdf(d2)


# ---------------------------------------------------------------------------
# Paramètres
# ---------------------------------------------------------------------------
S0        = 100.0
K         = 100.0   # ATM
T         = 1.0
R         = 0.05
Q         = 0.0
SIGMA_TRUE = 0.25

# ---------------------------------------------------------------------------
# 1. Prix BS à σ=0.25
# ---------------------------------------------------------------------------
call_price = bs_call(S0, K, R, SIGMA_TRUE, T)

print("=" * 60)
print("  VOLATILITÉ IMPLICITE — Inversion Black-Scholes")
print("=" * 60)
print(f"  Paramètres : S0={S0}, K={K}, T={T}Y, r={R}, q={Q}, σ_vrai={SIGMA_TRUE}")
print(f"\n--- 1. Prix BS à σ={SIGMA_TRUE} ---")
print(f"  Prix BS (call ATM) : {call_price:.8f}")

# ---------------------------------------------------------------------------
# 2. Inversion par k.implied_volatility
# ---------------------------------------------------------------------------
iv = k.implied_volatility(call_price, S0, K, T, R, Q)
err = abs(iv - SIGMA_TRUE)

print(f"\n--- 2. Vol implicite extraite par k.implied_volatility ---")
print(f"  σ implicite  : {iv:.8f}")
print(f"  σ vrai       : {SIGMA_TRUE}")
print(f"  |σ_imp − σ_vrai| : {err:.2e}")

assert err < 1e-3, f"Erreur vol implicite trop grande : {err:.2e}"
print(f"  [OK] Erreur < 1e-3")

# ---------------------------------------------------------------------------
# 3. Vérification sur plusieurs strikes (surface plate)
# ---------------------------------------------------------------------------
strikes = [85.0, 90.0, 95.0, 100.0, 105.0, 110.0, 115.0]
print(f"\n--- 3. Surface de vol implicite (σ constant = {SIGMA_TRUE}) ---")
print(f"  {'Strike':>8}  {'Prix BS':>10}  {'σ_imp':>10}  {'Erreur':>10}")
print(f"  {'-'*45}")

for strike in strikes:
    price = bs_call(S0, strike, R, SIGMA_TRUE, T)
    iv_k  = k.implied_volatility(price, S0, strike, T, R, Q)
    err_k = abs(iv_k - SIGMA_TRUE)
    ok    = "OK" if err_k < 1e-3 else "FAIL"
    print(f"  {strike:>8.1f}  {price:>10.6f}  {iv_k:>10.8f}  {err_k:>9.2e}  {ok}")
    assert err_k < 1e-3, f"K={strike}: erreur {err_k:.2e}"

print(f"\n  [OK] Surface de vol implicite plate — tous les strikes < 1e-3")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 60)
print("  RÉSUMÉ")
print("=" * 60)
print(f"  σ vrai           : {SIGMA_TRUE}")
print(f"  σ implicite ATM  : {iv:.8f}")
print(f"  Erreur           : {err:.2e}")
print("  Tous les asserts sont verts — script OK")
