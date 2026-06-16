"""
Option asiatique à fenêtre de fixings — average_over(a, b)

Intuition économique :
  En pratique, les options asiatiques sur matières premières ou FX
  n'utilisent pas toujours la moyenne sur toute la durée de vie.
  On définit une *fenêtre de fixings* [a, b] (ex. : derniers 6 mois).

  · Fenêtre courte en fin de période  → fixings proches de S_T
    ⟹ option « presque vanille », plus chère que la moyenne globale.
  · Moyenne globale [0, T] → lissage maximal, prix le plus bas.

  Ce script compare :
    A) Moyenne globale  average(S)          sur [0, 1]
    B) Moyenne fenêtrée average_over(S,0.5,1.0)  sur [0.5, 1]

  On s'attend à : prix B (fenêtré) ≥ prix A (global)
  car la fenêtre courte capte moins de lissage.

Ce script :
  - Construit les deux options asiatiques
  - Affiche les prix et intervalles de confiance
  - Compare et commente la différence
"""

import math
import kontract as k

# ---------------------------------------------------------------------------
# Référence Black-Scholes (call vanille, pour contextualiser)
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
WIN_A, WIN_B = 0.5, 1.0   # fenêtre = seconde moitié de l'année
N_PATHS = 150_000
SEED = 42
STEPS = 100    # grille fine — nécessaire pour la moyenne fenêtrée

print("=" * 65)
print("  KONTRACT — Asiatique global vs asiatique fenêtré")
print(f"  Fenêtre : [{WIN_A}, {WIN_B}]  vs  [0, {T}]")
print("=" * 65)

# ---------------------------------------------------------------------------
# Modèle GBM
# ---------------------------------------------------------------------------
model = k.GBM(s0=S0, sigma=SIGMA, r=R, asset="X")
S = k.S("X")

# ---------------------------------------------------------------------------
# 1. Asiatique global : moyenne sur [0, T]
# ---------------------------------------------------------------------------
global_payoff = (k.average(S) - K).clip(0.0) * k.one(k.USD)
global_call   = global_payoff @ k.at(T)
res_global    = global_call.price(model, n_paths=N_PATHS, seed=SEED, steps_per_year=STEPS)

print(f"\n--- 1. Asiatique global — moyenne sur [0, {T}] ---")
print(f"  Prix MC      : {res_global.price:.4f} ± {res_global.std_error:.4f}")
print(f"  IC 95%%       : [{res_global.ci95_low:.4f}, {res_global.ci95_high:.4f}]")

# ---------------------------------------------------------------------------
# 2. Asiatique fenêtré : moyenne sur [0.5, 1.0]
# ---------------------------------------------------------------------------
win_payoff = (k.average_over(S, WIN_A, WIN_B) - K).clip(0.0) * k.one(k.USD)
win_call   = win_payoff @ k.at(T)
res_win    = win_call.price(model, n_paths=N_PATHS, seed=SEED, steps_per_year=STEPS)

print(f"\n--- 2. Asiatique fenêtré — moyenne sur [{WIN_A}, {WIN_B}] ---")
print(f"  Prix MC      : {res_win.price:.4f} ± {res_win.std_error:.4f}")
print(f"  IC 95%%       : [{res_win.ci95_low:.4f}, {res_win.ci95_high:.4f}]")

# ---------------------------------------------------------------------------
# 3. Référence : call vanille
# ---------------------------------------------------------------------------
bs_van = bs_call(S0, K, R, SIGMA, T)
vanilla_payoff = (S - K).clip(0.0) * k.one(k.USD)
vanilla_call   = vanilla_payoff @ k.at(T)
res_van        = vanilla_call.price(model, n_paths=N_PATHS, seed=SEED, steps_per_year=STEPS)

print(f"\n--- 3. Référence : call vanille ATM ---")
print(f"  Prix MC      : {res_van.price:.4f} ± {res_van.std_error:.4f}")
print(f"  Prix BS      : {bs_van:.4f}")

# ---------------------------------------------------------------------------
# 4. Tableau comparatif
# ---------------------------------------------------------------------------
print(f"\n--- 4. Tableau comparatif ---")
print(f"  {'Produit':<30}  {'Prix':>8}  {'± σ_MC':>8}")
print(f"  {'-'*50}")
print(f"  {'Call vanille ATM':<30}  {res_van.price:>8.4f}  {res_van.std_error:>8.4f}")
print(f"  {'Asiatique global  [0,1]':<30}  {res_global.price:>8.4f}  {res_global.std_error:>8.4f}")
print(f"  {f'Asiatique fenêtré [{WIN_A},{WIN_B}]':<30}  {res_win.price:>8.4f}  {res_win.std_error:>8.4f}")

ecart = res_win.price - res_global.price
print(f"\n  Écart fenêtré − global : {ecart:+.4f}")
print(f"  Interprétation : la fenêtre [{WIN_A},{WIN_B}] couvre seulement la")
print(f"  2ᵉ moitié de l'année ; la moyenne porte sur moins de points,")
print(f"  ce qui réduit moins la volatilité effective → prix plus élevé.")

# ---------------------------------------------------------------------------
# Assertions
# ---------------------------------------------------------------------------
tol = 3 * (res_global.std_error + res_win.std_error)
assert res_win.price >= res_global.price - tol, (
    f"Le prix fenêtré ({res_win.price:.4f}) devrait être ≥ global ({res_global.price:.4f}) "
    f"(tol={tol:.4f})"
)
assert res_global.price < res_van.price + tol, (
    f"L'asiatique global devrait être < vanille (Jensen)"
)
print("\n  [OK] Asiatique fenêtré ≥ asiatique global (moins de lissage)")
print("  [OK] Asiatique global < vanille (Jensen)")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 65)
print("  RÉSUMÉ")
print("=" * 65)
print(f"  Call vanille         : {res_van.price:.4f}  (BS: {bs_van:.4f})")
print(f"  Asiatique global     : {res_global.price:.4f}  (lissage maximal)")
print(f"  Asiatique fenêtré    : {res_win.price:.4f}  (fenêtre [{WIN_A},{WIN_B}])")
print(f"  Surcoût fenêtrage    : {ecart:+.4f}")
print(f"  Tous les asserts sont verts — script OK")
