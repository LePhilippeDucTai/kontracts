"""
Réduction de variance : antithétique et variable de contrôle.

Compare trois estimateurs MC sur un call ATM 1 an :
  1. Monte-Carlo simple
  2. Variables antithétiques (antithetic=True)
  3. Variable de contrôle    (control_variate=True)

Même seed, même nombre de chemins. On vérifie que :
  • l'erreur-standard est réduite avec chaque technique
  • les prix restent cohérents (à quelques std_error les uns des autres)
"""

import math
import kontract as k

# ── Paramètres ─────────────────────────────────────────────────────────────
S0, K, r, sigma, T = 100.0, 100.0, 0.05, 0.20, 1.0
N_PATHS = 80_000
SEED = 42


# ── Black-Scholes de référence ─────────────────────────────────────────────
def N_cdf(x: float) -> float:
    return 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))


def bs_call(S: float, K: float, r: float, sigma: float, T: float) -> float:
    d1 = (math.log(S / K) + (r + 0.5 * sigma ** 2) * T) / (sigma * math.sqrt(T))
    d2 = d1 - sigma * math.sqrt(T)
    return S * N_cdf(d1) - K * math.exp(-r * T) * N_cdf(d2)


prix_bs = bs_call(S0, K, r, sigma, T)
print(f"Prix Black-Scholes de référence : {prix_bs:.4f}")
print(f"Paramètres : S={S0}, K={K}, r={r}, σ={sigma}, T={T}, n_paths={N_PATHS}, seed={SEED}\n")

# ── Pricing avec les trois méthodes ────────────────────────────────────────
modele = k.GBM(s0=S0, sigma=sigma, r=r, asset="X")
call = k.european_call("X", K, T, k.USD)

res_simple = call.price(modele, n_paths=N_PATHS, seed=SEED,
                        antithetic=False, control_variate=False)
res_antith = call.price(modele, n_paths=N_PATHS, seed=SEED,
                        antithetic=True,  control_variate=False)
res_ctrl   = call.price(modele, n_paths=N_PATHS, seed=SEED,
                        antithetic=False, control_variate=True)

# ── Affichage ──────────────────────────────────────────────────────────────
print(f"{'Méthode':<25}  {'Prix MC':>9}  {'Std-error':>10}  "
      f"{'Réduction σ':>12}  {'|Prix-BS|/BS':>13}")
print("-" * 75)

sigma_ref = res_simple.std_error

for label, res in [
    ("MC simple",            res_simple),
    ("Antithétique",         res_antith),
    ("Variable de contrôle", res_ctrl),
]:
    reduc = sigma_ref / res.std_error if res.std_error > 0 else float("inf")
    err_rel = abs(res.price - prix_bs) / prix_bs
    print(f"{label:<25}  {res.price:>9.4f}  {res.std_error:>10.6f}  "
          f"{reduc:>12.2f}×  {err_rel:>13.4%}")

# ── Vérifications ──────────────────────────────────────────────────────────
# L'antithétique réduit l'erreur-standard
assert res_antith.std_error < res_simple.std_error, (
    f"Antithétique : std_error non réduit "
    f"({res_antith.std_error:.6f} >= {res_simple.std_error:.6f})"
)

# La variable de contrôle réduit l'erreur-standard (davantage encore)
assert res_ctrl.std_error < res_simple.std_error, (
    f"Variable de contrôle : std_error non réduit "
    f"({res_ctrl.std_error:.6f} >= {res_simple.std_error:.6f})"
)

# Les prix restent cohérents avec BS (tolérance lâche : 5 × std_error_simple)
tol = 5.0 * res_simple.std_error
for label, res in [
    ("MC simple",            res_simple),
    ("Antithétique",         res_antith),
    ("Variable de contrôle", res_ctrl),
]:
    assert abs(res.price - prix_bs) < tol, (
        f"{label} : prix trop éloigné de BS "
        f"({res.price:.4f} vs {prix_bs:.4f}, tol={tol:.4f})"
    )

print(f"\nRéduction σ antithétique      : {sigma_ref / res_antith.std_error:.2f}×")
print(f"Réduction σ variable contrôle : {sigma_ref / res_ctrl.std_error:.2f}×")
print("\n✓ Toutes les assertions passent : réduction de variance vérifiée.")
