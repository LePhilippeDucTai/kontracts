"""
Straddle et Strangle.

Un straddle ATM est la somme d'un call et d'un put de même strike et maturité :
    Straddle = Call(K) + Put(K)
    Payoff = |S_T - K|

Un strangle est un call OTM + put OTM (strikes différents) :
    Strangle = Call(K_high) + Put(K_low),  K_high > S0 > K_low
    Payoff = max(S_T - K_high, 0) + max(K_low - S_T, 0)

Le strangle coûte moins cher que le straddle mais nécessite un mouvement
plus important pour être rentable.

Ce script illustre :
- k.straddle (catalogue) vs call + put manuels
- Strangle construit via k.european_call + k.european_put
- Comparaison des payoffs et des primes
"""

import math
import kontract as k


def _ncdf(x: float) -> float:
    return 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))


def bs_call(s: float, strike: float, r: float, sigma: float, t: float) -> float:
    d1 = (math.log(s / strike) + (r + 0.5 * sigma**2) * t) / (sigma * math.sqrt(t))
    d2 = d1 - sigma * math.sqrt(t)
    return s * _ncdf(d1) - strike * math.exp(-r * t) * _ncdf(d2)


def bs_put(s: float, strike: float, r: float, sigma: float, t: float) -> float:
    d1 = (math.log(s / strike) + (r + 0.5 * sigma**2) * t) / (sigma * math.sqrt(t))
    d2 = d1 - sigma * math.sqrt(t)
    return strike * math.exp(-r * t) * _ncdf(-d2) - s * _ncdf(-d1)


# ---------------------------------------------------------------------------
# Paramètres
# ---------------------------------------------------------------------------
S0    = 100.0
K_ATM = 100.0
K_LOW = 90.0      # put OTM pour le strangle
K_HIGH = 110.0    # call OTM pour le strangle
T     = 1.0
R     = 0.05
SIGMA = 0.20
N     = 100_000
SEED  = 42

MODEL = k.GBM(s0=S0, sigma=SIGMA, r=R, asset="X")

print("=" * 60)
print("  KONTRACT — Straddle et Strangle")
print("=" * 60)
print(f"\n  S0={S0}, T={T}, r={R}, σ={SIGMA}")

# ---------------------------------------------------------------------------
# 1. Straddle ATM — catalogue vs manuel vs BS
# ---------------------------------------------------------------------------
print("\n--- 1. Straddle ATM (K=100) ---")
straddle_cat = k.straddle("X", K_ATM, T, k.USD)
call_atm     = k.european_call("X", K_ATM, T, k.USD)
put_atm      = k.european_put("X", K_ATM, T, k.USD)
straddle_man = call_atm + put_atm

r_str_cat = straddle_cat.price(MODEL, n_paths=N, seed=SEED)
r_str_man = straddle_man.price(MODEL, n_paths=N, seed=SEED)
r_call    = call_atm.price(MODEL, n_paths=N, seed=SEED)
r_put     = put_atm.price(MODEL, n_paths=N, seed=SEED)

bs_c = bs_call(S0, K_ATM, R, SIGMA, T)
bs_p = bs_put(S0, K_ATM, R, SIGMA, T)
bs_strad = bs_c + bs_p

print(f"  Straddle catalogue (MC)     : {r_str_cat.price:.4f} ± {r_str_cat.std_error:.4f}")
print(f"  Straddle manuel (MC)        : {r_str_man.price:.4f} ± {r_str_man.std_error:.4f}")
print(f"  Call + Put BS               : {bs_strad:.4f}  ({bs_c:.4f} + {bs_p:.4f})")
print(f"  Différence catalogue-manuel : {abs(r_str_cat.price - r_str_man.price):.6f}")

assert abs(r_str_cat.price - bs_strad) / bs_strad < 0.02
assert abs(r_str_man.price - bs_strad) / bs_strad < 0.02
print("  [OK] Straddle MC ≈ BS (tolérance 2%)")

# ---------------------------------------------------------------------------
# 2. Strangle OTM (K_low=90, K_high=110)
# ---------------------------------------------------------------------------
print("\n--- 2. Strangle OTM (put K=90, call K=110) ---")
put_90   = k.european_put("X", K_LOW, T, k.USD)
call_110 = k.european_call("X", K_HIGH, T, k.USD)
strangle = put_90 + call_110

r_strangle = strangle.price(MODEL, n_paths=N, seed=SEED)
bs_strang  = bs_put(S0, K_LOW, R, SIGMA, T) + bs_call(S0, K_HIGH, R, SIGMA, T)

print(f"  Put OTM (K=90) BS    : {bs_put(S0, K_LOW, R, SIGMA, T):.4f}")
print(f"  Call OTM (K=110) BS  : {bs_call(S0, K_HIGH, R, SIGMA, T):.4f}")
print(f"  Strangle BS          : {bs_strang:.4f}")
print(f"  Strangle MC          : {r_strangle.price:.4f} ± {r_strangle.std_error:.4f}")
assert abs(r_strangle.price - bs_strang) / bs_strang < 0.03
print("  [OK] Strangle MC ≈ BS (tolérance 3%)")

# ---------------------------------------------------------------------------
# 3. Comparaison straddle vs strangle
# ---------------------------------------------------------------------------
print("\n--- 3. Straddle vs Strangle ---")
print(f"  Straddle  ATM (K=100)          : {r_str_cat.price:.4f}  (BS: {bs_strad:.4f})")
print(f"  Strangle OTM (K=90/110)        : {r_strangle.price:.4f}  (BS: {bs_strang:.4f})")
print(f"  Prime straddle - strangle      : {r_str_cat.price - r_strangle.price:.4f}")
print()
print("  Interprétation :")
print("  - Le straddle coûte plus cher (strikes ATM, délai de BE plus court)")
print("  - Le strangle coûte moins (strikes OTM, nécessite un mouvement plus fort)")
print(f"  - Point de breakeven straddle  : ≈ {K_ATM - r_str_cat.price:.1f} ou {K_ATM + r_str_cat.price:.1f}")
print(f"  - Point de breakeven strangle  : ≈ {K_LOW - r_strangle.price:.1f} ou {K_HIGH + r_strangle.price:.1f}")

assert r_str_cat.price > r_strangle.price, "Straddle doit coûter plus que strangle"
print("  [OK] Prix straddle > strangle")

# ---------------------------------------------------------------------------
# 4. Sensibilité à la volatilité — les deux produits longue vol
# ---------------------------------------------------------------------------
print("\n--- 4. Sensibilité à σ (produits longue vol) ---")
print(f"  {'σ':>6}  {'Straddle BS':>12}  {'Strangle BS':>12}  {'Ratio':>8}")
print("  " + "-" * 42)
for sigma_val in [0.10, 0.15, 0.20, 0.30, 0.40]:
    str_bs  = bs_call(S0, K_ATM, R, sigma_val, T) + bs_put(S0, K_ATM, R, sigma_val, T)
    stg_bs  = bs_call(S0, K_HIGH, R, sigma_val, T) + bs_put(S0, K_LOW, R, sigma_val, T)
    print(f"  {sigma_val:6.2f}  {str_bs:12.4f}  {stg_bs:12.4f}  {str_bs/stg_bs:8.4f}")

# ---------------------------------------------------------------------------
# 5. Vérification payoff terminal (profil de gains)
# ---------------------------------------------------------------------------
print("\n--- 5. Profil payoff straddle ATM (théorique) ---")
print(f"  {'S_T':>6}  {'Payoff straddle':>16}  {'Profit (net prime)':>20}")
premium = bs_strad
for s_t in [70, 80, 90, 100, 110, 120, 130]:
    payoff = abs(s_t - K_ATM)
    profit = payoff - premium
    print(f"  {s_t:6.1f}  {payoff:16.2f}  {profit:20.2f}")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 60)
print("  RÉSUMÉ")
print("=" * 60)
print(f"  Straddle ATM MC  : {r_str_cat.price:.4f}  (BS: {bs_strad:.4f})")
print(f"  Strangle OTM MC  : {r_strangle.price:.4f}  (BS: {bs_strang:.4f})")
print(f"  Écart prime      : {r_str_cat.price - r_strangle.price:.4f}")
print("  Tous les asserts sont verts — script OK")
