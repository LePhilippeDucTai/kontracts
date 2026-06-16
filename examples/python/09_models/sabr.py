"""
Modèle SABR — backbone CEV, skew (ρ) et smile (ν)
==================================================
Illustre les quatre paramètres du modèle SABR (Hagan et al. 2002) :

    dS = α S^β dW_S
    dα = ν α dW_α       corr(dW_S, dW_α) = ρ

Implémentation : schéma d'Euler en log-espace pour S et log-normal exact
pour α, avec browniens corrélés via décomposition de Cholesky.

Démonstrations :
  1. alpha contrôle le niveau de vol — deux alpha (0.18 vs 0.22) simulant
     deux marchés avec vol implicite différente.
  2. beta < 1 (CEV) vs beta = 1 (log-normal) pour comparer la richesse OTM.
  3. rho et nu : impact sur le skew et le smile de volatilité implicite.
     - ρ < 0 (actions) → puts OTM plus chers que calls OTM (skew négatif).
     - ν > 0 → smile (options OTM plus chères qu'avec vol plate).
"""

import math
import kontract as k


# ---------------------------------------------------------------------------
# Formule Black-Scholes de référence
# ---------------------------------------------------------------------------

def bs_call(S0: float, K: float, T: float, r: float, sigma: float) -> float:
    d1 = (math.log(S0 / K) + (r + 0.5 * sigma**2) * T) / (sigma * math.sqrt(T))
    d2 = d1 - sigma * math.sqrt(T)
    N = lambda x: 0.5 * (1 + math.erf(x / math.sqrt(2)))
    return S0 * N(d1) - K * math.exp(-r * T) * N(d2)

def bs_put(S0: float, K: float, T: float, r: float, sigma: float) -> float:
    return bs_call(S0, K, T, r, sigma) - S0 + K * math.exp(-r * T)


# ---------------------------------------------------------------------------
# Paramètres communs
# ---------------------------------------------------------------------------
S0, T, r = 100.0, 1.0, 0.05
PATHS  = 80_000
STEPS  = 100
SEED   = 42

S = k.S("X")
call_atm = (S - k.const_(100.0)).clip(0.0) * k.one(k.USD) @ k.at(T)
call_otm = (S - k.const_(110.0)).clip(0.0) * k.one(k.USD) @ k.at(T)
put_otm  = (k.const_(90.0) - S).clip(0.0)  * k.one(k.USD) @ k.at(T)

print("=" * 68)
print("  Modèle SABR — skew de vol et impact de alpha")
print("=" * 68)

# ---------------------------------------------------------------------------
# Partie 1 : Deux marchés avec alpha différent (skew implicite bas vs haut)
# ---------------------------------------------------------------------------
# alpha_bas  : marché avec vol implicite basse (ex. période calme)
# alpha_haut : marché avec vol implicite haute (ex. stress)
alpha_bas  = 0.18   # vol ATM effective ~ 18 %
alpha_haut = 0.22   # vol ATM effective ~ 22 %

def pricer_alpha(alpha_val: float) -> dict:
    m = k.sabr(spot=S0, alpha=alpha_val, beta=1.0, nu=0.4, rho=-0.5, r=r, asset="X")
    ra = call_atm.price(m, n_paths=PATHS, seed=SEED, steps_per_year=STEPS)
    rc = call_otm.price(m, n_paths=PATHS, seed=SEED, steps_per_year=STEPS)
    rp = put_otm.price( m, n_paths=PATHS, seed=SEED, steps_per_year=STEPS)
    return {"atm": ra.price, "co": rc.price, "po": rp.price, "se": ra.std_error}

p_bas  = pricer_alpha(alpha_bas)
p_haut = pricer_alpha(alpha_haut)

print()
print("Partie 1 — Niveau de vol implicite : alpha bas (18 %) vs alpha haut (22 %)")
print(f"  Paramètres communs : beta=1.0, nu=0.4, rho=-0.5")
print()
print(f"{'Produit':<22} {'BS(alpha_bas)':<16} {'SABR alpha=0.18':<18} {'SABR alpha=0.22'}")
print("-" * 72)
for label, K_strike, bs_fn in [
    ("Call ATM K=100", 100.0, bs_call),
    ("Call OTM K=110", 110.0, bs_call),
    ("Put  OTM K=90 ",  90.0, bs_put),
]:
    bs_v = bs_fn(S0, K_strike, T, r, alpha_bas)
    col_bas  = p_bas["atm"]  if "ATM" in label else (p_bas["co"]  if "110" in label else p_bas["po"])
    col_haut = p_haut["atm"] if "ATM" in label else (p_haut["co"] if "110" in label else p_haut["po"])
    print(f"  {label:<20} {bs_v:<16.4f} {col_bas:<18.4f} {col_haut:.4f}")

print()
print(f"  Hausse relative call ATM  : {(p_haut['atm']-p_bas['atm'])/p_bas['atm']*100:+.1f}%")
print(f"  Hausse relative call OTM  : {(p_haut['co']-p_bas['co'])/p_bas['co']*100:+.1f}%")
print(f"  Hausse relative put  OTM  : {(p_haut['po']-p_bas['po'])/p_bas['po']*100:+.1f}%")

# ---------------------------------------------------------------------------
# Partie 2 : Rôle de beta — vol backbone log-normal vs sub-log-normal (CEV)
# ---------------------------------------------------------------------------
# Pour comparer à vol ATM constante, on recalibre alpha selon alpha_eff = alpha * F^(1-beta)
# → alpha_recal = 0.20 * F^(1-beta) = 0.20 * 100^(1-beta)
print()
print("Partie 2 — Rôle de beta (backbone CEV), alpha recalibré pour ATM ~ 20 %")
print(f"  alpha_recal = 0.20 × S0^(1-beta)")
print()
print(f"{'beta':<8} {'alpha_eff':<12} {'Call ATM':<12} {'Call OTM K=110':<18} {'Put OTM K=90'}")
print("-" * 60)

bs_atm_ref = bs_call(S0, 100.0, T, r, 0.20)
bs_co_ref  = bs_call(S0, 110.0, T, r, 0.20)
bs_po_ref  = bs_put( S0,  90.0, T, r, 0.20)

betas = [1.0, 0.8, 0.5]
for beta in betas:
    alpha_eff = 0.20 * (S0 ** (1 - beta))
    m = k.sabr(spot=S0, alpha=alpha_eff, beta=beta, nu=0.0, rho=0.0, r=r, asset="X")
    ra = call_atm.price(m, n_paths=PATHS, seed=SEED, steps_per_year=STEPS)
    rc = call_otm.price(m, n_paths=PATHS, seed=SEED, steps_per_year=STEPS)
    rp = put_otm.price( m, n_paths=PATHS, seed=SEED, steps_per_year=STEPS)
    print(f"  {beta:<6} {alpha_eff:<12.4f} {ra.price:<12.4f} {rc.price:<18.4f} {rp.price:.4f}")

print()
print(f"  BS(20%) référence :       {bs_atm_ref:<12.4f} {bs_co_ref:<18.4f} {bs_po_ref:.4f}")
print()
print("Commentaire :")
print("  - beta = 1   : backbone log-normal (GBM), vol implicite plate en strike.")
print("  - beta < 1   : backbone CEV, la vol implicite augmente pour les strikes bas.")
print("    → puts OTM plus chers, calls OTM moins chers vs beta=1 à ATM égale.")
print("  - Nu (vol de vol) et rho (corrélation spot-vol) affinent le smile mais")
print("    beta est le premier déterminant de la pente du skew de vol implicite.")
print("  - En pratique, les traders actions calibrent beta ~ 0.5–0.8 pour")
print("    reproduire le skew observé sur le marché des options.")

# ---------------------------------------------------------------------------
# Partie 3 — Rôle de rho et nu : skew et smile de vol implicite
# ---------------------------------------------------------------------------
# Strikes symétriques autour de l'ATM (S0=100) : K=110 (call OTM) et K=90 (put OTM)
call_110 = (S - k.const_(110.0)).clip(0.0) * k.one(k.USD) @ k.at(T)
put_90   = (k.const_(90.0) - S).clip(0.0)  * k.one(k.USD) @ k.at(T)

PATHS3 = 120_000  # plus de chemins pour détecter des effets de skew modérés

# ρ = -0.7 : corrélation négative spot-vol (scénario actions typique)
m_neg = k.sabr(spot=S0, alpha=0.20, beta=1.0, nu=0.40, rho=-0.70, r=r, asset="X")
p_call_neg = call_110.price(m_neg, n_paths=PATHS3, seed=SEED, steps_per_year=STEPS).price
p_put_neg  = put_90.price(  m_neg, n_paths=PATHS3, seed=SEED, steps_per_year=STEPS).price

# ρ = +0.7 : corrélation positive (matières premières, etc.)
m_pos = k.sabr(spot=S0, alpha=0.20, beta=1.0, nu=0.40, rho=+0.70, r=r, asset="X")
p_call_pos = call_110.price(m_pos, n_paths=PATHS3, seed=SEED, steps_per_year=STEPS).price
p_put_pos  = put_90.price(  m_pos, n_paths=PATHS3, seed=SEED, steps_per_year=STEPS).price

# nu = 0 vs nu = 0.4 (avec ρ=0 pour isoler l'effet smile)
m_nu0 = k.sabr(spot=S0, alpha=0.20, beta=1.0, nu=0.00, rho=0.0, r=r, asset="X")
m_nu4 = k.sabr(spot=S0, alpha=0.20, beta=1.0, nu=0.40, rho=0.0, r=r, asset="X")
p_call_nu0 = call_110.price(m_nu0, n_paths=PATHS3, seed=SEED, steps_per_year=STEPS).price
p_call_nu4 = call_110.price(m_nu4, n_paths=PATHS3, seed=SEED, steps_per_year=STEPS).price

print()
print("Partie 3 — Impact de rho (skew) et nu (smile)")
print(f"  Même option comparée à différents rho/nu (S0={S0}, forward≈105.1)")
print(f"  Le skew se lit en comparant UNE option entre deux rho (pas put vs call,")
print(f"  car K=110 et K=90 ne sont pas symétriques autour du forward).")
print()
print(f"{'Option':<18} {'rho=-0.7':>10} {'rho=+0.7':>10}  Effet du skew")
print("-" * 70)
print(f"  Call OTM K=110   {p_call_neg:>10.4f} {p_call_pos:>10.4f}  aile droite ↑ si rho>0")
print(f"  Put  OTM K=90    {p_put_neg:>10.4f} {p_put_pos:>10.4f}  aile gauche ↑ si rho<0")
print()
print(f"{'Option':<18} {'nu=0':>10} {'nu=0.4':>10}  Effet du smile")
print("-" * 70)
print(f"  Call OTM K=110   {p_call_nu0:>10.4f} {p_call_nu4:>10.4f}  OTM ↑ avec nu>0")
print()
print("Commentaire :")
print("  - rho < 0 : hausse spot → baisse vol → aile GAUCHE renchérie (put 90 plus cher).")
print("  - rho > 0 : hausse spot → hausse vol → aile DROITE renchérie (call 110 plus cher).")
print("  - nu > 0  : vol elle-même aléatoire → queues plus épaisses → options OTM")
print("    plus chères qu'avec une vol plate (ν=0 ≡ GBM).")

# ---------------------------------------------------------------------------
# Assertions
# ---------------------------------------------------------------------------
# Prix augmentent avec alpha
assert p_haut["atm"] > p_bas["atm"], "Call ATM doit augmenter avec alpha"
assert p_haut["co"]  > p_bas["co"],  "Call OTM doit augmenter avec alpha"
assert p_haut["po"]  > p_bas["po"],  "Put  OTM doit augmenter avec alpha"

# Hausse proportionnelle des OTM plus grande que ATM (levier de vol)
lever_atm = (p_haut["atm"] - p_bas["atm"]) / p_bas["atm"]
lever_co  = (p_haut["co"]  - p_bas["co"])  / p_bas["co"]
lever_po  = (p_haut["po"]  - p_bas["po"])  / p_bas["po"]
assert lever_co > lever_atm, (
    f"Les OTM calls devraient être plus sensibles à alpha que l'ATM "
    f"(co={lever_co:.3f}, atm={lever_atm:.3f})"
)
assert lever_po > lever_atm, (
    f"Les OTM puts devraient être plus sensibles à alpha que l'ATM "
    f"(po={lever_po:.3f}, atm={lever_atm:.3f})"
)

# Skew rho : l'aile DROITE (call 110) se renchérit quand rho passe de −0.7 à +0.7.
assert p_call_pos > p_call_neg, (
    f"call OTM K=110 devrait croître avec rho "
    f"(rho=-0.7 → {p_call_neg:.4f}, rho=+0.7 → {p_call_pos:.4f})"
)
# Skew rho : l'aile GAUCHE (put 90) se renchérit quand rho passe de +0.7 à −0.7.
assert p_put_neg > p_put_pos, (
    f"put OTM K=90 devrait décroître avec rho "
    f"(rho=-0.7 → {p_put_neg:.4f}, rho=+0.7 → {p_put_pos:.4f})"
)
# Smile nu : ν > 0 renchérit les options OTM vs vol plate (ν=0).
assert p_call_nu4 > p_call_nu0, (
    f"nu=0.4 : call OTM ({p_call_nu4:.4f}) devrait dépasser nu=0 ({p_call_nu0:.4f})"
)

print()
print("Assertions OK — sensibilité à alpha, levier OTM, skew rho, smile nu conformes.")
