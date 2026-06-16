"""
Options rainbow — Best-of et Worst-of sur 2 actifs
===================================================
Price des options sur le maximum (best-of) et le minimum (worst-of)
de deux actifs corrélés.

Définitions :
  Best-of  : Payoff = max(max(S1_T, S2_T) - K, 0)
  Worst-of : Payoff = max(min(S1_T, S2_T) - K, 0)

Relations théoriques :
  1. best_of >= call_mono_actif  (le max est toujours >= chaque actif)
  2. best_of >= worst_of        (le max >= le min)
  3. best_of + worst_of = call(S1) + call(S2)  (identité de décomposition)
     car max(a,b) + min(a,b) = a + b

La décomposition (3) fournit une borne : si call(S1) ≈ call(S2) ≈ C_mono,
alors best_of + worst_of ≈ 2 × C_mono.

Impact de la corrélation :
  - ρ↑  → best_of ↓  (actifs évoluent ensemble → max moins extrême)
           worst_of ↑ (le faible actif suit le bon)
  - ρ↓  → best_of ↑, worst_of ↓
"""

import math
import kontract as k


# ---------------------------------------------------------------------------
# Formule Black-Scholes de référence (call européen)
# ---------------------------------------------------------------------------

def bs_call(S0: float, K: float, T: float, r: float, sigma: float) -> float:
    d1 = (math.log(S0 / K) + (r + 0.5 * sigma**2) * T) / (sigma * math.sqrt(T))
    d2 = d1 - sigma * math.sqrt(T)
    N = lambda x: 0.5 * (1 + math.erf(x / math.sqrt(2)))
    return S0 * N(d1) - K * math.exp(-r * T) * N(d2)


# ---------------------------------------------------------------------------
# Paramètres
# ---------------------------------------------------------------------------
s0    = 100.0   # spot initial (identique pour S1 et S2)
sigma = 0.20    # vol (identique pour S1 et S2)
rho   = 0.50    # corrélation S1-S2
r     = 0.05    # taux sans risque
K     = 100.0   # strike
T     = 1.0     # maturité

PATHS = 100_000
STEPS =  50
SEED  =  42

# ---------------------------------------------------------------------------
# Modèle GBM corrélé
# ---------------------------------------------------------------------------
factors = [
    k.GbmFactor("S1", s0, r, sigma),
    k.GbmFactor("S2", s0, r, sigma),
]
corr  = [[1.0, rho], [rho, 1.0]]
model = k.correlated_gbm(factors, corr, r=r)

# ---------------------------------------------------------------------------
# Contrats
# ---------------------------------------------------------------------------
S1, S2 = k.S("S1"), k.S("S2")

best_of  = (S1.max(S2) - k.const_(K)).clip(0.0) * k.one(k.USD) @ k.at(T)
worst_of = (S1.min(S2) - k.const_(K)).clip(0.0) * k.one(k.USD) @ k.at(T)
call_s1  = (S1 - k.const_(K)).clip(0.0)         * k.one(k.USD) @ k.at(T)
call_s2  = (S2 - k.const_(K)).clip(0.0)         * k.one(k.USD) @ k.at(T)

# ---------------------------------------------------------------------------
# Pricing
# ---------------------------------------------------------------------------
def price(contrat):
    return contrat.price(model, n_paths=PATHS, seed=SEED, steps_per_year=STEPS)

res_best  = price(best_of)
res_worst = price(worst_of)
res_s1    = price(call_s1)
res_s2    = price(call_s2)

bs_mono = bs_call(s0, K, T, r, sigma)

# ---------------------------------------------------------------------------
# Affichage
# ---------------------------------------------------------------------------
print("=" * 68)
print("  Options Rainbow — Best-of et Worst-of sur 2 actifs")
print("=" * 68)
print(f"\nParamètres : S1_0=S2_0={s0}, σ={sigma}, ρ={rho}, r={r}, K={K}, T={T}")
print(f"  n_paths={PATHS:,}, steps/an={STEPS}, seed={SEED}")
print()

print(f"Prix BS mono-actif BS(σ=20%) : {bs_mono:.4f}")
print()
print(f"{'Produit':<28} {'Prix MC':<12} {'σ_MC':<10} {'IC 95%'}")
print("-" * 65)

for label, res in [
    ("Call mono S1  K=100", res_s1),
    ("Call mono S2  K=100", res_s2),
    ("Best-of  max(S1,S2)-K", res_best),
    ("Worst-of min(S1,S2)-K", res_worst),
]:
    print(f"  {label:<26} {res.price:<12.4f} {res.std_error:<10.4f} "
          f"[{res.ci95_low:.4f}, {res.ci95_high:.4f}]")

print()
# Vérification de la décomposition best_of + worst_of ≈ call_s1 + call_s2
somme_rainbow  = res_best.price  + res_worst.price
somme_monos    = res_s1.price    + res_s2.price
ecart_decomp   = abs(somme_rainbow - somme_monos) / somme_monos

print("Décomposition max + min = S1 + S2 :")
print(f"  best_of + worst_of = {somme_rainbow:.4f}")
print(f"  call_S1 + call_S2  = {somme_monos:.4f}")
print(f"  Ecart relatif      = {ecart_decomp*100:.2f}%")

print()
print("Commentaire :")
print("  - best_of >= call_mono : max(S1,S2) ≥ Si → l'option best-of")
print(f"    vaut {res_best.price:.4f} vs BS_mono = {bs_mono:.4f}  "
      f"(+{(res_best.price/bs_mono-1)*100:.1f}%)")
print("  - worst_of <= call_mono : min(S1,S2) ≤ Si → l'option worst-of")
print(f"    vaut {res_worst.price:.4f} vs BS_mono = {bs_mono:.4f}  "
      f"({(res_worst.price/bs_mono-1)*100:.1f}%)")
print("  - Identité : best_of + worst_of = call_S1 + call_S2 (vérifiée)")
print(f"    car max(a,b) + min(a,b) = a + b.")
print(f"  - ρ = {rho} : corrélation modérée → best_of significativement")
print("    > worst_of mais pas autant qu'avec ρ = 0.")

# Sensibilité à la corrélation (analytique uniquement)
print()
print("Sensibilité à la corrélation (illustration par MC rapide) :")
print(f"  {'ρ':<8} {'best-of':<12} {'worst-of':<12} {'best+worst'}")
print("  " + "-" * 45)
for rho_val in [0.0, 0.3, 0.5, 0.7, 1.0]:
    corr_v = [[1.0, rho_val], [rho_val, 1.0]]
    m_v    = k.correlated_gbm(factors, corr_v, r=r)
    rb     = best_of.price( m_v, n_paths=50_000, seed=SEED, steps_per_year=STEPS)
    rw     = worst_of.price(m_v, n_paths=50_000, seed=SEED, steps_per_year=STEPS)
    print(f"  {rho_val:<8} {rb.price:<12.4f} {rw.price:<12.4f} {rb.price+rw.price:.4f}")

# ---------------------------------------------------------------------------
# Assertions
# ---------------------------------------------------------------------------
# best_of >= worst_of
assert res_best.price > res_worst.price, (
    f"best_of ({res_best.price:.4f}) doit être > worst_of ({res_worst.price:.4f})"
)

# best_of >= call mono-actif (tolérance MC)
mc_tol = 3 * max(res_best.std_error, res_s1.std_error)
assert res_best.price >= res_s1.price - mc_tol, (
    f"best_of ({res_best.price:.4f}) doit être >= call S1 ({res_s1.price:.4f}) - MC_tol"
)
assert res_best.price >= res_s2.price - mc_tol, (
    f"best_of ({res_best.price:.4f}) doit être >= call S2 ({res_s2.price:.4f}) - MC_tol"
)

# Décomposition best_of + worst_of ≈ call_s1 + call_s2
assert ecart_decomp < 0.02, (
    f"Décomposition rainbow : écart {ecart_decomp*100:.2f}% (tolérance 2%)"
)

print()
print("Assertions OK — best_of ≥ worst_of, best_of ≥ call_mono, identité best+worst=C1+C2.")
