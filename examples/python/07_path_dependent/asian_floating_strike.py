"""
Option asiatique à strike flottant — max(S_T - moyenne(S), 0)

Intuition économique :
  Contrairement au call à frappe fixe, le strike est ici endogène :
  il correspond à la moyenne du spot sur toute la durée de vie.
  L'acheteur profite si le prix *final* est supérieur à la moyenne.
  Ce produit est utilisé par les traders qui veulent capturer la tendance
  haussière en fin de période, nette du niveau moyen d'entrée.

  Propriétés :
    · Prix toujours > 0 car il existe toujours des scénarios où S_T > moyenne.
    · Payoff = 0 si le spot est constant — aucune tendance = aucun gain.
    · Diffère du call à frappe fixe : ici K = avg(S), variable aléatoire.

Ce script :
  - Construit le call flottant via (S - k.average(S)).clip(0.0)
  - Compare avec le call asiatique à frappe fixe ATM
  - Affiche prix et intervalles de confiance
  - Vérifie que le prix est strictement positif
"""

import math
import kontract as k

# ---------------------------------------------------------------------------
# Référence Black-Scholes (call vanille, pour contexte)
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
STEPS = 100   # grille fine pour une moyenne précise

print("=" * 65)
print("  KONTRACT — Option asiatique à strike flottant")
print("  max(S_T - moyenne(S), 0)")
print("=" * 65)

# ---------------------------------------------------------------------------
# Modèle GBM
# ---------------------------------------------------------------------------
model = k.GBM(s0=S0, sigma=SIGMA, r=R, asset="X")
S = k.S("X")

# ---------------------------------------------------------------------------
# 1. Call asiatique à frappe fixe (référence)
# ---------------------------------------------------------------------------
fixed_payoff = (k.average(S) - K).clip(0.0) * k.one(k.USD)
fixed_call   = fixed_payoff @ k.at(T)
res_fixed    = fixed_call.price(model, n_paths=N_PATHS, seed=SEED, steps_per_year=STEPS)

print(f"\n--- 1. Call asiatique à frappe fixe (K={K}) — référence ---")
print(f"  Prix MC      : {res_fixed.price:.4f} ± {res_fixed.std_error:.4f}")
print(f"  IC 95%%       : [{res_fixed.ci95_low:.4f}, {res_fixed.ci95_high:.4f}]")

# ---------------------------------------------------------------------------
# 2. Call asiatique à strike flottant  max(S_T - avg(S), 0)
# ---------------------------------------------------------------------------
float_payoff = (S - k.average(S)).clip(0.0) * k.one(k.USD)
float_call   = float_payoff @ k.at(T)
res_float    = float_call.price(model, n_paths=N_PATHS, seed=SEED, steps_per_year=STEPS)

print(f"\n--- 2. Call asiatique à strike flottant ---")
print(f"  Payoff = max(S_T - moyenne(S_0..T), 0)")
print(f"  Prix MC      : {res_float.price:.4f} ± {res_float.std_error:.4f}")
print(f"  IC 95%%       : [{res_float.ci95_low:.4f}, {res_float.ci95_high:.4f}]")

# ---------------------------------------------------------------------------
# 3. Contexte : call vanille
# ---------------------------------------------------------------------------
bs_van = bs_call(S0, K, R, SIGMA, T)
print(f"\n--- 3. Contexte : call vanille ATM ---")
print(f"  Prix BS (référence) : {bs_van:.4f}")
print(f"  Call fixe  / vanille : {res_fixed.price:.4f} / {bs_van:.4f}")
print(f"  Call flott. / vanille: {res_float.price:.4f} / {bs_van:.4f}")

# ---------------------------------------------------------------------------
# 4. Analyse
# ---------------------------------------------------------------------------
print(f"\n--- 4. Analyse économique ---")
print(f"  Le call flottant ne dépend pas de K : le gain mesure")
print(f"  la sur-performance du prix final par rapport à la moyenne.")
print(f"  Pour σ={SIGMA}, horizon T={T}Y :")
print(f"    · Frappe fixe  = {res_fixed.price:.4f}  (moins cher : moyenne < S_T en espérance)")
print(f"    · Strike flott = {res_float.price:.4f}  (capte la tendance haussière de fin de période)")

# ---------------------------------------------------------------------------
# Assertions
# ---------------------------------------------------------------------------
assert res_float.price > 0, (
    f"Le call flottant doit valoir > 0, obtenu {res_float.price:.4f}"
)
tol = 3 * res_float.std_error
assert res_float.price > tol, (
    f"Prix flottant statistiquement nul ({res_float.price:.4f} ≤ 3σ={tol:.4f})"
)
print("\n  [OK] Prix du call flottant strictement positif")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 65)
print("  RÉSUMÉ")
print("=" * 65)
print(f"  Call vanille  ATM 1Y     : {bs_van:.4f}  (BS analytique)")
print(f"  Call asiatique fixe      : {res_fixed.price:.4f}")
print(f"  Call asiatique flottant  : {res_float.price:.4f}")
print(f"  Tous les asserts sont verts — script OK")
