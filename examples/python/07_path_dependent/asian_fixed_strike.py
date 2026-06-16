"""
Option asiatique à frappe fixe — max(moyenne(S) - K, 0)

Intuition économique :
  Un call asiatique remplace le prix final S_T par la moyenne arithmétique
  du spot sur la durée de vie de l'option.  La moyenne lisse les chocs :
  sa variance est strictement inférieure à celle de S_T (inégalité de Jensen).
  Le call asiatique vaut donc MOINS que le call vanille de même frappe K.

  Prix asiatique < Prix vanille  (sauf si σ → 0, cas limite d'égalité)

Ce script :
  - Construit le call asiatique via k.average(S)
  - Le compare au call vanille ATM de référence (Monte-Carlo + Black-Scholes)
  - Affiche l'écart absolu et relatif
  - Vérifie l'inégalité de Jensen
"""

import math
import kontract as k

# ---------------------------------------------------------------------------
# Référence Black-Scholes
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
STEPS = 100   # grille fine — la moyenne porte sur tous les steps

print("=" * 65)
print("  KONTRACT — Option asiatique à frappe fixe")
print("  max(moyenne(S) - K, 0)")
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

res_van = vanilla_call.price(model, n_paths=N_PATHS, seed=SEED, steps_per_year=STEPS)
bs_van  = bs_call(S0, K, R, SIGMA, T)

print(f"\n--- 1. Call vanille ATM (K={K}, T={T}Y) ---")
print(f"  Prix MC      : {res_van.price:.4f} ± {res_van.std_error:.4f}")
print(f"  IC 95%%       : [{res_van.ci95_low:.4f}, {res_van.ci95_high:.4f}]")
print(f"  Prix BS      : {bs_van:.4f}")
print(f"  Erreur rel.  : {abs(res_van.price - bs_van) / bs_van * 100:.2f}%")

# ---------------------------------------------------------------------------
# 2. Call asiatique à frappe fixe  max(avg(S) - K, 0)
# ---------------------------------------------------------------------------
asian_payoff = (k.average(S) - K).clip(0.0) * k.one(k.USD)
asian_call   = asian_payoff @ k.at(T)

res_asi = asian_call.price(model, n_paths=N_PATHS, seed=SEED, steps_per_year=STEPS)

print(f"\n--- 2. Call asiatique à frappe fixe (K={K}, T={T}Y) ---")
print(f"  Prix MC      : {res_asi.price:.4f} ± {res_asi.std_error:.4f}")
print(f"  IC 95%%       : [{res_asi.ci95_low:.4f}, {res_asi.ci95_high:.4f}]")

# ---------------------------------------------------------------------------
# 3. Comparaison & propriété de Jensen
# ---------------------------------------------------------------------------
ecart_abs = res_van.price - res_asi.price
ecart_rel = ecart_abs / res_van.price * 100

print(f"\n--- 3. Comparaison (Jensen : asiatique < vanille) ---")
print(f"  Vanille       : {res_van.price:.4f}")
print(f"  Asiatique     : {res_asi.price:.4f}")
print(f"  Écart absolu  : {ecart_abs:.4f}")
print(f"  Écart relatif : {ecart_rel:.1f}%")
print(f"  Interprétation: la moyenne réduit la volatilité effective,")
print(f"  d'où un prix inférieur d'environ {ecart_rel:.0f}% pour σ={SIGMA}.")

tol = 3 * (res_van.std_error + res_asi.std_error)   # marge Monte-Carlo à 3σ
assert res_asi.price < res_van.price + tol, (
    f"Inégalité de Jensen violée : asiatique={res_asi.price:.4f} "
    f">= vanille={res_van.price:.4f} (tol={tol:.4f})"
)
print("  [OK] Inégalité de Jensen vérifiée : asiatique < vanille")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 65)
print("  RÉSUMÉ")
print("=" * 65)
print(f"  Call vanille  ATM 1Y  : {res_van.price:.4f}  (BS: {bs_van:.4f})")
print(f"  Call asiatique fixe   : {res_asi.price:.4f}")
print(f"  Réduction de prix     : {ecart_abs:.4f}  ({ecart_rel:.1f}%)")
print(f"  Tous les asserts sont verts — script OK")
