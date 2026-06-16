"""
Options européennes call et put.

Comparaison Monte-Carlo vs Black-Scholes analytique, et vérification
de la parité call-put :

    C - P = S0 · e^{-qT} - K · e^{-rT}

(sans dividende q=0 : C - P = S0 - K · e^{-rT})

Ce script illustre :
- k.european_call / k.european_put (catalogue)
- Construction manuelle équivalente via DSL
- Parité call-put numérique
- Sensibilité à la moneyness (ITM, ATM, OTM)
"""

import math
import kontract as k

# ---------------------------------------------------------------------------
# Black-Scholes analytique
# ---------------------------------------------------------------------------

def _ncdf(x: float) -> float:
    return 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))


def bs_call(s: float, strike: float, r: float, sigma: float, t: float, q: float = 0.0) -> float:
    d1 = (math.log(s / strike) + (r - q + 0.5 * sigma**2) * t) / (sigma * math.sqrt(t))
    d2 = d1 - sigma * math.sqrt(t)
    return s * math.exp(-q * t) * _ncdf(d1) - strike * math.exp(-r * t) * _ncdf(d2)


def bs_put(s: float, strike: float, r: float, sigma: float, t: float, q: float = 0.0) -> float:
    d1 = (math.log(s / strike) + (r - q + 0.5 * sigma**2) * t) / (sigma * math.sqrt(t))
    d2 = d1 - sigma * math.sqrt(t)
    return strike * math.exp(-r * t) * _ncdf(-d2) - s * math.exp(-q * t) * _ncdf(-d1)


# ---------------------------------------------------------------------------
# Paramètres
# ---------------------------------------------------------------------------
S0    = 100.0
K     = 100.0    # ATM
T     = 1.0
R     = 0.05
SIGMA = 0.20
N     = 100_000
SEED  = 42
DISC  = math.exp(-R * T)

MODEL = k.GBM(s0=S0, sigma=SIGMA, r=R, asset="X")

print("=" * 60)
print("  KONTRACT — Options Européennes Call & Put")
print("=" * 60)
print(f"\n  S0={S0}, K={K}, T={T}, r={R}, σ={SIGMA}")

# ---------------------------------------------------------------------------
# 1. Call européen ATM — catalogue vs DSL vs BS
# ---------------------------------------------------------------------------
print("\n--- 1. Call ATM : catalogue vs DSL vs Black-Scholes ---")
call_cat = k.european_call("X", K, T, k.USD)
call_dsl = ((k.S("X") - K).clip(0.0) * k.one(k.USD)) @ k.at(T)

r_cat = call_cat.price(MODEL, n_paths=N, seed=SEED)
r_dsl = call_dsl.price(MODEL, n_paths=N, seed=SEED)
analytic_c = bs_call(S0, K, R, SIGMA, T)

print(f"  Call catalogue MC : {r_cat.price:.4f} ± {r_cat.std_error:.4f}")
print(f"  Call DSL MC       : {r_dsl.price:.4f} ± {r_dsl.std_error:.4f}")
print(f"  Black-Scholes     : {analytic_c:.4f}")
assert abs(r_cat.price - analytic_c) / analytic_c < 0.02
assert abs(r_dsl.price - analytic_c) / analytic_c < 0.02
print("  [OK] MC ≈ BS (tolérance 2%)")

# ---------------------------------------------------------------------------
# 2. Put européen ATM
# ---------------------------------------------------------------------------
print("\n--- 2. Put ATM ---")
put_cat = k.european_put("X", K, T, k.USD)
put_dsl = ((K - k.S("X")).clip(0.0) * k.one(k.USD)) @ k.at(T)

r_put_cat = put_cat.price(MODEL, n_paths=N, seed=SEED)
r_put_dsl = put_dsl.price(MODEL, n_paths=N, seed=SEED)
analytic_p = bs_put(S0, K, R, SIGMA, T)

print(f"  Put catalogue MC  : {r_put_cat.price:.4f} ± {r_put_cat.std_error:.4f}")
print(f"  Put DSL MC        : {r_put_dsl.price:.4f} ± {r_put_dsl.std_error:.4f}")
print(f"  Black-Scholes     : {analytic_p:.4f}")
assert abs(r_put_cat.price - analytic_p) / analytic_p < 0.02
print("  [OK] Put MC ≈ BS (tolérance 2%)")

# ---------------------------------------------------------------------------
# 3. Parité call-put : C - P = S0 - K·e^{-rT}
# ---------------------------------------------------------------------------
print("\n--- 3. Parité call-put : C - P = S0 - K·e^{-rT} ---")
parity_mc  = r_cat.price - r_put_cat.price
parity_th  = S0 - K * DISC
analytic_parity = analytic_c - analytic_p

print(f"  C (MC)    : {r_cat.price:.4f}")
print(f"  P (MC)    : {r_put_cat.price:.4f}")
print(f"  C-P (MC)  : {parity_mc:.4f}")
print(f"  S0-K·e^{{-rT}} = {S0} - {K}·{DISC:.4f} = {parity_th:.4f}")
print(f"  C-P (BS)  : {analytic_parity:.4f}")
assert abs(parity_mc - parity_th) < 0.2, f"Parité violée : {parity_mc:.4f} vs {parity_th:.4f}"
print("  [OK] Parité call-put vérifiée")

# ---------------------------------------------------------------------------
# 4. Sensibilité à la moneyness
# ---------------------------------------------------------------------------
print("\n--- 4. Sensibilité à la moneyness ---")
print(f"  {'Strike':>8}  {'Moneyness':>10}  {'Call MC':>10}  {'Call BS':>10}  {'Put MC':>10}  {'Put BS':>10}")
print("  " + "-" * 68)
for strike in [80, 90, 100, 110, 120]:
    K_val = float(strike)
    moneyness = S0 / K_val
    c_mc  = k.european_call("X", K_val, T, k.USD).price(MODEL, n_paths=N, seed=SEED)
    p_mc  = k.european_put("X", K_val, T, k.USD).price(MODEL, n_paths=N, seed=SEED)
    c_bs  = bs_call(S0, K_val, R, SIGMA, T)
    p_bs  = bs_put(S0, K_val, R, SIGMA, T)
    label = "ITM" if moneyness > 1 else ("ATM" if moneyness == 1 else "OTM")
    print(f"  {K_val:8.0f}  {label:>10}  {c_mc.price:10.4f}  {c_bs:10.4f}  {p_mc.price:10.4f}  {p_bs:10.4f}")

# ---------------------------------------------------------------------------
# 5. Sensibilité à la volatilité
# ---------------------------------------------------------------------------
print("\n--- 5. Sensibilité de l'ATM call à σ ---")
print(f"  {'σ':>6}  {'Call MC':>10}  {'Call BS':>10}  {'Erreur rel.':>12}")
print("  " + "-" * 44)
for sigma_val in [0.10, 0.20, 0.30, 0.40, 0.50]:
    model_s = k.GBM(s0=S0, sigma=sigma_val, r=R, asset="X")
    r_s = k.european_call("X", K, T, k.USD).price(model_s, n_paths=N, seed=SEED)
    bs_s = bs_call(S0, K, R, sigma_val, T)
    rel = abs(r_s.price - bs_s) / bs_s
    print(f"  {sigma_val:6.2f}  {r_s.price:10.4f}  {bs_s:10.4f}  {rel:12.4f}")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 60)
print("  RÉSUMÉ")
print("=" * 60)
print(f"  Call ATM (MC)    : {r_cat.price:.4f}  (BS: {analytic_c:.4f})")
print(f"  Put  ATM (MC)    : {r_put_cat.price:.4f}  (BS: {analytic_p:.4f})")
print(f"  Parité C-P       : {parity_mc:.4f}  (théorique: {parity_th:.4f})")
print(f"  Erreur parité    : {abs(parity_mc - parity_th):.4f}")
print("  Tous les asserts sont verts — script OK")
