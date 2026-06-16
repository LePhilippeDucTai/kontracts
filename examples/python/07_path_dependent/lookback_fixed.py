"""
Option lookback à frappe fixe — max(max(S) - K, 0)

Intuition économique :
  Le lookback à frappe fixe donne à l'acheteur le *maximum* du spot
  atteint sur toute la durée de vie de l'option, plutôt que le prix
  final S_T.

  Puisque max(S) ≥ S_T toujours (par définition), le lookback est
  TOUJOURS plus cher que le call vanille de même frappe K :

      Prix lookback fixe > Prix call vanille ATM

  Ce surcoût est la « prime d'hindsight » : l'acheteur peut vendre
  rétrospectivement au pic, ce que le vendeur doit couvrir.

Ce script :
  - Construit le lookback via k.running_max(S)
  - Compare avec le call vanille de même paramètres
  - Calcule la prime d'hindsight absolue et relative
  - Vérifie la dominance running_max ≥ S_T
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
STEPS = 100   # grille fine — le max discret s'approche du max continu

print("=" * 65)
print("  KONTRACT — Lookback à frappe fixe")
print("  max(running_max(S) - K, 0)")
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
print(f"  Erreur rel.  : {abs(res_van.price - bs_van) / bs_van * 100:.2f}%")

# ---------------------------------------------------------------------------
# 2. Lookback à frappe fixe  max(running_max(S) - K, 0)
# ---------------------------------------------------------------------------
lb_payoff = (k.running_max(S) - K).clip(0.0) * k.one(k.USD)
lb_call   = lb_payoff @ k.at(T)
res_lb    = lb_call.price(model, n_paths=N_PATHS, seed=SEED, steps_per_year=STEPS)

print(f"\n--- 2. Lookback à frappe fixe (K={K}, T={T}Y) ---")
print(f"  Payoff = max(running_max(S) - {K}, 0)")
print(f"  Prix MC      : {res_lb.price:.4f} ± {res_lb.std_error:.4f}")
print(f"  IC 95%%       : [{res_lb.ci95_low:.4f}, {res_lb.ci95_high:.4f}]")

# ---------------------------------------------------------------------------
# 3. Prime d'hindsight
# ---------------------------------------------------------------------------
prime_abs = res_lb.price - res_van.price
prime_rel = prime_abs / res_van.price * 100

print(f"\n--- 3. Prime d'hindsight (lookback vs vanille) ---")
print(f"  Vanille       : {res_van.price:.4f}")
print(f"  Lookback      : {res_lb.price:.4f}")
print(f"  Prime abs.    : +{prime_abs:.4f}")
print(f"  Prime rel.    : +{prime_rel:.1f}%")
print(f"  Interprétation : l'acheteur « regarde en arrière » et reçoit")
print(f"  le pic plutôt que le prix final → surcoût de {prime_rel:.0f}% ici.")
print(f"  La prime augmente avec σ (un marché plus volatile a des pics")
print(f"  plus élevés par rapport à S_T).")

# ---------------------------------------------------------------------------
# Assertions
# ---------------------------------------------------------------------------
tol = 3 * (res_lb.std_error + res_van.std_error)
assert res_lb.price > res_van.price - tol, (
    f"Lookback ({res_lb.price:.4f}) doit être > vanille ({res_van.price:.4f}), "
    f"tol={tol:.4f}"
)
assert prime_abs > 0, (
    f"Prime d'hindsight négative : {prime_abs:.4f}"
)
print("\n  [OK] Lookback > vanille (prime d'hindsight positive)")

# ---------------------------------------------------------------------------
# 4. Sensibilité à la volatilité (illustration)
# ---------------------------------------------------------------------------
print(f"\n--- 4. Sensibilité à σ (prime d'hindsight) ---")
print(f"  {'σ':>6}  {'Vanille':>10}  {'Lookback':>10}  {'Prime':>10}  {'Prime%':>8}")
for sigma_test in [0.10, 0.20, 0.30, 0.40]:
    m = k.GBM(s0=S0, sigma=sigma_test, r=R, asset="X")
    r_van = vanilla_call.price(m, n_paths=80_000, seed=SEED, steps_per_year=STEPS)
    r_lb  = lb_call.price(m, n_paths=80_000, seed=SEED, steps_per_year=STEPS)
    p = r_lb.price - r_van.price
    print(f"  {sigma_test:>6.2f}  {r_van.price:>10.4f}  {r_lb.price:>10.4f}  "
          f"{p:>10.4f}  {p/r_van.price*100:>7.1f}%")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 65)
print("  RÉSUMÉ")
print("=" * 65)
print(f"  Call vanille  ATM 1Y  : {res_van.price:.4f}  (BS: {bs_van:.4f})")
print(f"  Lookback fixe ATM 1Y  : {res_lb.price:.4f}")
print(f"  Prime d'hindsight     : +{prime_abs:.4f}  (+{prime_rel:.1f}%)")
print(f"  Tous les asserts sont verts — script OK")
