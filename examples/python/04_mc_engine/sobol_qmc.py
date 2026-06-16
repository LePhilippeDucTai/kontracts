"""
Quasi-Monte-Carlo randomisé avec k.sobol_gbm.

Compare deux estimateurs du prix d'un call ATM 1 an :
  1. GBM standard (Monte-Carlo pseudo-aléatoire)
  2. Sobol GBM    (quasi-Monte-Carlo randomisé : suite de Halton + décalage
                   aléatoire de Cranley-Patterson piloté par la graine)

Les deux estimateurs sont NON BIAISÉS (prix ≈ Black-Scholes), mais le rQMC
converge nettement plus vite. La bonne mesure de ce gain n'est PAS le
`std_error` d'un seul tirage (l'erreur QMC n'est pas une variance d'échantillon
classique) mais la DISPERSION de l'estimateur sur plusieurs décalages
aléatoires : on lance N runs avec des graines différentes et on compare l'écart
-type des prix obtenus. Le rQMC se resserre beaucoup plus que le MC pseudo.

Construction : chaque pas de temps j utilise une base de Halton première
distincte (2, 3, 5, …) ; un décalage aléatoire par dimension (graine) randomise
la suite tout en préservant la distribution U[0,1) exacte → estimateur sans biais.
"""

import math
import statistics
import kontract as k

# ── Paramètres ─────────────────────────────────────────────────────────────
S0, K, r, sigma, T = 100.0, 100.0, 0.05, 0.20, 1.0
SEED = 42


# ── Black-Scholes de référence ─────────────────────────────────────────────
def N_cdf(x: float) -> float:
    return 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))


def bs_call(S: float, K: float, r: float, sigma: float, T: float) -> float:
    d1 = (math.log(S / K) + (r + 0.5 * sigma ** 2) * T) / (sigma * math.sqrt(T))
    d2 = d1 - sigma * math.sqrt(T)
    return S * N_cdf(d1) - K * math.exp(-r * T) * N_cdf(d2)


prix_bs = bs_call(S0, K, r, sigma, T)
print(f"Prix Black-Scholes (référence) : {prix_bs:.4f}")
print(f"Paramètres : S={S0}, K={K}, r={r}, σ={sigma}, T={T}\n")

# ── Modèles ────────────────────────────────────────────────────────────────
call = k.european_call("X", K, T, k.USD)
m_gbm = k.GBM(s0=S0, sigma=sigma, r=r, asset="X")
m_sobol = k.sobol_gbm(spot=S0, sigma=sigma, r=r, asset="X")

# ── 1. Justesse : les deux estimateurs reproduisent BS ─────────────────────
N_REF = 65_536
res_gbm = call.price(m_gbm, n_paths=N_REF, seed=SEED)
res_sobol = call.price(m_sobol, n_paths=N_REF, seed=SEED)
err_gbm = abs(res_gbm.price - prix_bs) / prix_bs
err_sobol = abs(res_sobol.price - prix_bs) / prix_bs

print("1. Justesse (n_paths = 65 536, une graine)")
print(f"{'Modèle':<12}  {'Prix':>9}  {'|Prix-BS|/BS':>13}")
print("-" * 40)
print(f"{'GBM (MC)':<12}  {res_gbm.price:>9.4f}  {err_gbm:>13.4%}")
print(f"{'Sobol QMC':<12}  {res_sobol.price:>9.4f}  {err_sobol:>13.4%}")

# ── 2. Gain rQMC : dispersion de l'estimateur sur plusieurs décalages ──────
# On répète le pricing avec plusieurs graines (décalages aléatoires indépendants)
# et on mesure l'écart-type des prix : c'est l'erreur effective de l'estimateur.
N_PATHS = 8_192
SEEDS = list(range(1, 21))  # 20 décalages indépendants

prices_sobol = [
    call.price(m_sobol, n_paths=N_PATHS, seed=s, steps_per_year=5).price for s in SEEDS
]
prices_gbm = [
    call.price(m_gbm, n_paths=N_PATHS, seed=s, steps_per_year=5).price for s in SEEDS
]

std_sobol = statistics.pstdev(prices_sobol)
std_gbm = statistics.pstdev(prices_gbm)
mean_sobol = statistics.mean(prices_sobol)
mean_gbm = statistics.mean(prices_gbm)
ratio = std_gbm / std_sobol if std_sobol > 0 else float("inf")

print(f"\n2. Dispersion de l'estimateur sur {len(SEEDS)} décalages "
      f"(n_paths = {N_PATHS})")
print(f"{'Modèle':<12}  {'Moyenne':>9}  {'Écart-type':>11}")
print("-" * 38)
print(f"{'GBM (MC)':<12}  {mean_gbm:>9.4f}  {std_gbm:>11.5f}")
print(f"{'Sobol rQMC':<12}  {mean_sobol:>9.4f}  {std_sobol:>11.5f}")
print(f"\n→ Réduction d'erreur effective (écart-type) : {ratio:.1f}×")

# ── 3. Convergence : l'erreur rQMC décroît plus vite en n ──────────────────
print("\n3. Convergence (écart-type sur 12 décalages selon n_paths)")
print(f"{'n_paths':>8}  {'std GBM':>10}  {'std Sobol':>11}  {'Ratio':>7}")
print("-" * 42)
for n in [1_024, 2_048, 4_096, 8_192]:
    seeds = list(range(1, 13))
    ps = [call.price(m_sobol, n_paths=n, seed=s, steps_per_year=5).price for s in seeds]
    pg = [call.price(m_gbm, n_paths=n, seed=s, steps_per_year=5).price for s in seeds]
    ss, sg = statistics.pstdev(ps), statistics.pstdev(pg)
    rr = sg / ss if ss > 0 else float("inf")
    print(f"{n:>8}  {sg:>10.5f}  {ss:>11.5f}  {rr:>6.1f}×")

# ── Vérifications ──────────────────────────────────────────────────────────
# 1. Les deux estimateurs sont non biaisés (< 1 % de BS).
assert err_gbm < 0.01, f"GBM : erreur relative trop grande ({err_gbm:.4%})"
assert err_sobol < 0.01, (
    f"Sobol : erreur relative trop grande ({err_sobol:.4%}) — biais détecté"
)
# 2. Les deux moyennes (sur décalages) restent centrées sur BS.
assert abs(mean_sobol - prix_bs) / prix_bs < 0.01, "Sobol rQMC doit être centré sur BS"
# 3. Le rQMC réduit nettement la dispersion de l'estimateur (gain ≥ 3×).
assert ratio > 3.0, (
    f"Sobol rQMC doit réduire l'écart-type d'au moins 3× (obtenu {ratio:.1f}×)"
)

print("\n✓ Assertions passées : prix Sobol non biaisé ET dispersion fortement réduite.")
