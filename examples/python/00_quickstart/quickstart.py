"""
Tour en 60 secondes de kontract.

Ce script illustre le cycle complet :
1. Construction d'un call européen ATM via le DSL compositionnel
2. Pricing Monte-Carlo sous GBM + comparaison Black-Scholes analytique
3. Ajout d'une barrière knock-out pour montrer la composition
4. Calcul des Greeks (delta, gamma, vega)
"""

import math
import kontract as k


# ---------------------------------------------------------------------------
# Référence analytique Black-Scholes
# ---------------------------------------------------------------------------

def _norm_cdf(x: float) -> float:
    return 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))


def bs_call(s: float, strike: float, r: float, sigma: float, t: float) -> float:
    d1 = (math.log(s / strike) + (r + 0.5 * sigma ** 2) * t) / (sigma * math.sqrt(t))
    d2 = d1 - sigma * math.sqrt(t)
    return s * _norm_cdf(d1) - strike * math.exp(-r * t) * _norm_cdf(d2)


def bs_put(s: float, strike: float, r: float, sigma: float, t: float) -> float:
    d1 = (math.log(s / strike) + (r + 0.5 * sigma ** 2) * t) / (sigma * math.sqrt(t))
    d2 = d1 - sigma * math.sqrt(t)
    return strike * math.exp(-r * t) * _norm_cdf(-d2) - s * _norm_cdf(-d1)


# ---------------------------------------------------------------------------
# Paramètres
# ---------------------------------------------------------------------------
S0, K, T, SIGMA, R = 100.0, 100.0, 1.0, 0.20, 0.05
BARRIER = 150.0
N_PATHS = 100_000
SEED = 42

print("=" * 60)
print("  KONTRACT — Tour en 60 secondes")
print("=" * 60)

# ---------------------------------------------------------------------------
# 1. Construction du call ATM via DSL
# ---------------------------------------------------------------------------
spot = k.S("X")
payoff = (spot - K).clip(0.0) * k.one(k.USD)
call = payoff @ k.at(T)

model = k.GBM(s0=S0, sigma=SIGMA, r=R, asset="X")

print("\n--- 1. Call européen ATM (DSL) ---")
res = call.price(model, n_paths=N_PATHS, seed=SEED)
analytic = bs_call(S0, K, R, SIGMA, T)
rel_err = abs(res.price - analytic) / analytic

print(f"  Prix MC      : {res.price:.4f} ± {res.std_error:.4f}")
print(f"  IC 95%%       : [{res.ci95_low:.4f}, {res.ci95_high:.4f}]")
print(f"  Prix BS      : {analytic:.4f}")
print(f"  Erreur rel.  : {rel_err:.4f}  ({rel_err*100:.2f}%)")

assert rel_err < 0.03, f"Erreur MC trop grande : {rel_err:.4f}"
print("  [OK] Erreur < 3%")

# ---------------------------------------------------------------------------
# 2. Barrière knock-out up-and-out (barrière H=150)
# ---------------------------------------------------------------------------
print("\n--- 2. Call up-and-out (H=150) via .until() ---")
knock_out = call.until(k.S("X") >= BARRIER)
res_ko = knock_out.price(model, n_paths=N_PATHS, seed=SEED, steps_per_year=100)

print(f"  Prix call vanille   : {res.price:.4f}")
print(f"  Prix call KO (H=150): {res_ko.price:.4f}")
print(f"  Rabais barrière     : {res.price - res_ko.price:.4f}")
assert res_ko.price < res.price, "Le call KO doit valoir moins que le call vanille"
print("  [OK] KO < vanille")

# ---------------------------------------------------------------------------
# 3. Greeks (GBM uniquement)
# ---------------------------------------------------------------------------
print("\n--- 3. Greeks du call ATM ---")
greeks = call.greeks(model, n_paths=200_000, seed=SEED)

print(f"  Prix   : {greeks.price:.4f}")
print(f"  Delta  : {greeks.delta:.4f}  (BS théorique ≈ 0.637)")
print(f"  Gamma  : {greeks.gamma:.5f}")
print(f"  Vega   : {greeks.vega:.4f}  (pour σ=0.20)")
print(f"  Rho    : {greeks.rho:.4f}")

assert 0.50 < greeks.delta < 0.80, "Delta ATM hors plage attendue"
assert greeks.gamma > 0, "Gamma doit être positif"
assert greeks.vega > 0, "Vega doit être positif"
print("  [OK] Greeks dans les plages attendues")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 60)
print("  RÉSUMÉ")
print("=" * 60)
print(f"  Call ATM 1Y MC   : {res.price:.4f}  (BS: {analytic:.4f})")
print(f"  Call KO H=150    : {res_ko.price:.4f}  (rabais: {res.price-res_ko.price:.4f})")
print(f"  Delta / Gamma    : {greeks.delta:.3f} / {greeks.gamma:.5f}")
print(f"  Vega             : {greeks.vega:.3f}")
print("  Tous les asserts sont verts — script OK")
