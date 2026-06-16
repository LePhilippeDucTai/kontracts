"""
Diagnostics Monte-Carlo : convergence en 1/√N.

Démontre que l'erreur-standard du pricer MC décroît proportionnellement
à 1/√n_paths. On price un call ATM 1 an (GBM standard) avec un nombre
croissant de chemins et on vérifie que std_error × √n_paths ≈ constante.
L'intervalle de confiance à 95 % est affiché et on vérifie qu'il contient
le prix Black-Scholes de référence.
"""

import math
import kontract as k

# ── Paramètres ─────────────────────────────────────────────────────────────
S0, K, r, sigma, T = 100.0, 100.0, 0.05, 0.20, 1.0
SEED = 42
N_LIST = [5_000, 20_000, 80_000, 320_000]


# ── Black-Scholes de référence ─────────────────────────────────────────────
def N_cdf(x: float) -> float:
    """Loi normale cumulée via math.erf."""
    return 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))


def bs_call(S: float, K: float, r: float, sigma: float, T: float) -> float:
    """Prix BS d'un call européen."""
    d1 = (math.log(S / K) + (r + 0.5 * sigma ** 2) * T) / (sigma * math.sqrt(T))
    d2 = d1 - sigma * math.sqrt(T)
    return S * N_cdf(d1) - K * math.exp(-r * T) * N_cdf(d2)


prix_bs = bs_call(S0, K, r, sigma, T)
print(f"Prix Black-Scholes de référence : {prix_bs:.4f}\n")

# ── Boucle sur n_paths ─────────────────────────────────────────────────────
modele = k.GBM(s0=S0, sigma=sigma, r=r, asset="X")
call = k.european_call("X", K, T, k.USD)

print(f"{'n_paths':>8}  {'prix MC':>9}  {'std_error':>10}  {'std*√n':>9}  "
      f"{'IC95 bas':>9}  {'IC95 haut':>10}  {'BS∈IC95':>8}")
print("-" * 75)

produits_sqrt_n = []

for n in N_LIST:
    res = call.price(modele, n_paths=n, seed=SEED)
    prod = res.std_error * math.sqrt(n)
    produits_sqrt_n.append(prod)
    dans_ic = res.ci95_low <= prix_bs <= res.ci95_high
    print(
        f"{n:>8}  {res.price:>9.4f}  {res.std_error:>10.5f}  {prod:>9.4f}  "
        f"{res.ci95_low:>9.4f}  {res.ci95_high:>10.4f}  {'✓' if dans_ic else '✗':>8}"
    )

# ── Vérifications ──────────────────────────────────────────────────────────
# 1) std_error × √n est à peu près constant (variation < 20 %)
ratio_max_min = max(produits_sqrt_n) / min(produits_sqrt_n)
assert ratio_max_min < 1.20, (
    f"std_error × √n varie trop : ratio max/min = {ratio_max_min:.3f}"
)

# 2) Le plus grand n_paths donne un IC95 qui contient le prix BS
res_gros = call.price(modele, n_paths=N_LIST[-1], seed=SEED)
assert res_gros.ci95_low <= prix_bs <= res_gros.ci95_high, (
    f"BS ({prix_bs:.4f}) hors de l'IC95 "
    f"[{res_gros.ci95_low:.4f}, {res_gros.ci95_high:.4f}]"
)

# 3) n_paths retourné correspond bien à ce qu'on a demandé
assert res_gros.n_paths == N_LIST[-1], (
    f"n_paths retourné ({res_gros.n_paths}) ≠ demandé ({N_LIST[-1]})"
)

print(f"\nRatio max/min de std_error×√n : {ratio_max_min:.3f}  (doit être < 1.20)")
print(f"n_paths retourné par PriceResult : {res_gros.n_paths}")
print("\n✓ Toutes les assertions passent : convergence MC en 1/√N confirmée.")
