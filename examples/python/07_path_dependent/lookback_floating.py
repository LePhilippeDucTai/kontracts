"""
Option lookback à strike flottant — S_T - min(S)

Intuition économique :
  Le lookback à strike flottant (put-based) donne à l'acheteur la
  différence entre le prix final et le *minimum* du spot sur la durée
  de vie.  L'acheteur profite d'avoir « acheté au plus bas ».

  Propriétés de dominance :
    1. Payoff = S_T - min(S) ≥ 0  toujours (min ≤ S_T par définition).
       Le prix est donc toujours strictement positif.

    2. S_T - min(S) ≥ max(S_T - K, 0) pour K ≤ min(S)  (domination).
       Plus généralement, le lookback flottant domine le call vanille
       ATM en prix car son payoff peut être plus grand dans tous les scénarios.

  Ce script :
    - Construit le lookback flottant via (S - k.running_min(S)).clip(0.0)
    - Compare avec le call vanille ATM
    - Vérifie prix > 0 et prix > vanille ATM (dominance)
    - Affiche la sensibilité à σ
"""

import math
import kontract as k

# ---------------------------------------------------------------------------
# Référence Black-Scholes (call vanille)
# ---------------------------------------------------------------------------

def _norm_cdf(x: float) -> float:
    return 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))


def bs_call(s: float, strike: float, r: float, sigma: float, t: float) -> float:
    d1 = (math.log(s / strike) + (r + 0.5 * sigma**2) * t) / (sigma * math.sqrt(t))
    d2 = d1 - sigma * math.sqrt(t)
    return s * _norm_cdf(d1) - strike * math.exp(-r * t) * _norm_cdf(d2)


# ---------------------------------------------------------------------------
# Paramètres
# ---------------------------------------------------------------------------
S0, K, T, SIGMA, R = 100.0, 100.0, 1.0, 0.20, 0.05
N_PATHS = 150_000
SEED = 42
STEPS = 100   # grille fine — le min discret doit approcher le min continu

print("=" * 65)
print("  KONTRACT — Lookback à strike flottant")
print("  S_T - running_min(S)  (toujours ≥ 0)")
print("=" * 65)

# ---------------------------------------------------------------------------
# Modèle GBM
# ---------------------------------------------------------------------------
model = k.GBM(s0=S0, sigma=SIGMA, r=R, asset="X")
S = k.S("X")

# ---------------------------------------------------------------------------
# 1. Call vanille de référence
# ---------------------------------------------------------------------------
vanilla_payoff = (S - K).clip(0.0) * k.one(k.USD)
vanilla_call   = vanilla_payoff @ k.at(T)
res_van        = vanilla_call.price(model, n_paths=N_PATHS, seed=SEED, steps_per_year=STEPS)
bs_van         = bs_call(S0, K, R, SIGMA, T)

print(f"\n--- 1. Call vanille ATM (K={K}, T={T}Y) ---")
print(f"  Prix MC      : {res_van.price:.4f} ± {res_van.std_error:.4f}")
print(f"  Prix BS      : {bs_van:.4f}")

# ---------------------------------------------------------------------------
# 2. Lookback à strike flottant  max(S_T - running_min(S), 0)
#    = S_T - running_min(S)  car S_T - min(S) ≥ 0 toujours
# ---------------------------------------------------------------------------
lb_payoff = (S - k.running_min(S)).clip(0.0) * k.one(k.USD)
lb_call   = lb_payoff @ k.at(T)
res_lb    = lb_call.price(model, n_paths=N_PATHS, seed=SEED, steps_per_year=STEPS)

print(f"\n--- 2. Lookback à strike flottant (T={T}Y) ---")
print(f"  Payoff = S_T - running_min(S)  [toujours ≥ 0]")
print(f"  Prix MC      : {res_lb.price:.4f} ± {res_lb.std_error:.4f}")
print(f"  IC 95%%       : [{res_lb.ci95_low:.4f}, {res_lb.ci95_high:.4f}]")

# ---------------------------------------------------------------------------
# 3. Comparaison et dominance
# ---------------------------------------------------------------------------
ecart_abs = res_lb.price - res_van.price
ecart_rel = ecart_abs / res_van.price * 100

print(f"\n--- 3. Comparaison & propriété de dominance ---")
print(f"  Vanille ATM         : {res_van.price:.4f}")
print(f"  Lookback flottant   : {res_lb.price:.4f}")
print(f"  Surcoût abs.        : +{ecart_abs:.4f}")
print(f"  Surcoût rel.        : +{ecart_rel:.1f}%")
print(f"  Interprétation : l'acheteur « achète au plus bas »,")
print(f"  ce qui est bien plus avantageux que d'acheter à K fixe.")
print(f"  La prime est d'environ +{ecart_rel:.0f}% par rapport au call vanille.")

# ---------------------------------------------------------------------------
# Assertions
# ---------------------------------------------------------------------------
tol = 3 * res_lb.std_error
assert res_lb.price > tol, (
    f"Prix lookback flottant statistiquement nul : {res_lb.price:.4f} ≤ 3σ={tol:.4f}"
)
tol2 = 3 * (res_lb.std_error + res_van.std_error)
assert res_lb.price > res_van.price - tol2, (
    f"Lookback flottant ({res_lb.price:.4f}) devrait dominer vanille ({res_van.price:.4f}), "
    f"tol={tol2:.4f}"
)
print("\n  [OK] Prix du lookback flottant strictement positif")
print("  [OK] Lookback flottant > call vanille ATM (dominance)")

# ---------------------------------------------------------------------------
# 4. Sensibilité à la volatilité
# ---------------------------------------------------------------------------
print(f"\n--- 4. Sensibilité à σ ---")
print(f"  {'σ':>6}  {'Vanille':>10}  {'LB flottant':>12}  {'Surcoût%':>10}")
for sigma_test in [0.10, 0.20, 0.30, 0.40]:
    m = k.GBM(s0=S0, sigma=sigma_test, r=R, asset="X")
    r_van = vanilla_call.price(m, n_paths=80_000, seed=SEED, steps_per_year=STEPS)
    r_lb  = lb_call.price(m, n_paths=80_000, seed=SEED, steps_per_year=STEPS)
    sur = (r_lb.price - r_van.price) / r_van.price * 100
    print(f"  {sigma_test:>6.2f}  {r_van.price:>10.4f}  {r_lb.price:>12.4f}  {sur:>9.1f}%")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 65)
print("  RÉSUMÉ")
print("=" * 65)
print(f"  Call vanille  ATM 1Y     : {res_van.price:.4f}  (BS: {bs_van:.4f})")
print(f"  Lookback flottant 1Y     : {res_lb.price:.4f}  (payoff = S_T - min(S))")
print(f"  Surcoût « meilleur achat »: +{ecart_abs:.4f}  (+{ecart_rel:.1f}%)")
print(f"  Tous les asserts sont verts — script OK")
