"""
Cost-of-carry et dividendes continus.

La formule du prix forward équitable avec dividende continu q est :

    F* = S0 · e^{(r-q)·T}

Un forward avec K=F* vaut 0. Avec un dividende q>0, le forward équitable
est plus bas que sans dividende, car les dividendes réduisent la valeur
de l'actif sous-jacent au fil du temps.

Ce script illustre :
- Le modèle GBM avec dividende continu (paramètre q)
- Le forward équitable F*(r,q) vs F*(r,0)
- Vérification que forward(K=F*) ≈ 0 dans les deux cas
- Impact du dividende sur le prix du forward
"""

import math
import kontract as k

S0    = 100.0
T     = 1.0
R     = 0.05
Q     = 0.03     # dividende continu
SIGMA = 0.20
N     = 80_000
SEED  = 42

MODEL_NO_DIV = k.GBM(s0=S0, sigma=SIGMA, r=R, q=0.0, asset="X")
MODEL_DIV    = k.GBM(s0=S0, sigma=SIGMA, r=R, q=Q,   asset="X")

F_NO_DIV = S0 * math.exp(R * T)
F_DIV    = S0 * math.exp((R - Q) * T)

DISC = math.exp(-R * T)

print("=" * 60)
print("  KONTRACT — Cost-of-carry et dividendes")
print("=" * 60)
print(f"\n  S0={S0}, r={R}, q={Q}, T={T}, σ={SIGMA}")
print(f"  F* (sans div) = S0·e^{{rT}}     = {F_NO_DIV:.4f}")
print(f"  F* (avec div) = S0·e^{{(r-q)T}} = {F_DIV:.4f}")
print(f"  Impact dividende sur F*      : {F_NO_DIV - F_DIV:.4f}")

# ---------------------------------------------------------------------------
# 1. Forward équitable sans dividende
# ---------------------------------------------------------------------------
print("\n--- 1. Forward(K=F*) sans dividende → ≈ 0 ---")
fwd_no_div = k.forward("X", F_NO_DIV, T, k.USD)
r_no_div = fwd_no_div.price(MODEL_NO_DIV, n_paths=N, seed=SEED)
print(f"  K = {F_NO_DIV:.4f}")
print(f"  Prix MC       : {r_no_div.price:.4f} ± {r_no_div.std_error:.4f}")
print(f"  IC 95%%        : [{r_no_div.ci95_low:.4f}, {r_no_div.ci95_high:.4f}]")
assert abs(r_no_div.price) < 2 * r_no_div.std_error * 3, \
    f"Forward équitable (sans div) trop loin de 0: {r_no_div.price}"
print("  [OK] Forward équitable ≈ 0")

# ---------------------------------------------------------------------------
# 2. Forward équitable avec dividende
# ---------------------------------------------------------------------------
print("\n--- 2. Forward(K=F*) avec dividende q=0.03 → ≈ 0 ---")
fwd_div = k.forward("X", F_DIV, T, k.USD)
r_div = fwd_div.price(MODEL_DIV, n_paths=N, seed=SEED)
print(f"  K = {F_DIV:.4f}")
print(f"  Prix MC       : {r_div.price:.4f} ± {r_div.std_error:.4f}")
print(f"  IC 95%%        : [{r_div.ci95_low:.4f}, {r_div.ci95_high:.4f}]")
assert abs(r_div.price) < 2 * r_div.std_error * 3, \
    f"Forward équitable (avec div) trop loin de 0: {r_div.price}"
print("  [OK] Forward équitable ≈ 0")

# ---------------------------------------------------------------------------
# 3. Impact du dividende : forward ATM (K=100) avec et sans dividende
# ---------------------------------------------------------------------------
print("\n--- 3. Impact du dividende : forward(K=100) ---")
fwd_atm_no_div = k.forward("X", 100.0, T, k.USD)
fwd_atm_div    = k.forward("X", 100.0, T, k.USD)

r_atm_no = fwd_atm_no_div.price(MODEL_NO_DIV, n_paths=N, seed=SEED)
r_atm_dv = fwd_atm_div.price(MODEL_DIV, n_paths=N, seed=SEED)

analytic_no = S0 - 100.0 * DISC
analytic_dv = S0 * math.exp(-Q * T) - 100.0 * DISC   # S0·e^{-qT} - K·e^{-rT}

print(f"  Sans dividende : MC={r_atm_no.price:.4f}  analytique={analytic_no:.4f}")
print(f"  Avec dividende : MC={r_atm_dv.price:.4f}  analytique={analytic_dv:.4f}")
print(f"  Réduction due au dividende : {r_atm_no.price - r_atm_dv.price:.4f}")
assert r_atm_dv.price < r_atm_no.price, "Le dividende doit réduire le prix du forward"
print("  [OK] dividende réduit bien le forward")

# ---------------------------------------------------------------------------
# 4. Sensibilité au dividende : plusieurs valeurs de q
# ---------------------------------------------------------------------------
print("\n--- 4. Sensibilité du forward ATM(K=S0) au dividende ---")
print(f"  {'q':>6}  {'F*':>10}  {'Fwd(K=100) MC':>16}  {'Fwd(K=100) anal.':>18}")
print("  " + "-" * 56)
for q_val in [0.0, 0.01, 0.03, 0.05, 0.08]:
    model_q = k.GBM(s0=S0, sigma=SIGMA, r=R, q=q_val, asset="X")
    f_star = S0 * math.exp((R - q_val) * T)
    fwd_k = k.forward("X", 100.0, T, k.USD)
    r_k = fwd_k.price(model_q, n_paths=N, seed=SEED)
    anal = S0 * math.exp(-q_val * T) - 100.0 * DISC
    print(f"  {q_val:6.2f}  {f_star:10.4f}  {r_k.price:16.4f}  {anal:18.4f}")

# ---------------------------------------------------------------------------
# 5. Vérification carry : le forward équitable intègre r et q
# ---------------------------------------------------------------------------
print("\n--- 5. Carry : F*(r,q) = S0·e^{(r-q)T} ---")
for r_val, q_val in [(0.05, 0.0), (0.05, 0.03), (0.02, 0.01), (0.10, 0.05)]:
    f_star = S0 * math.exp((r_val - q_val) * T)
    model_rq = k.GBM(s0=S0, sigma=SIGMA, r=r_val, q=q_val, asset="X")
    fwd_eq = k.forward("X", f_star, T, k.USD)
    r_eq = fwd_eq.price(model_rq, n_paths=N, seed=SEED)
    print(f"  r={r_val:.2f} q={q_val:.2f} → F*={f_star:.4f}  fwd(F*)={r_eq.price:+.4f}")
    assert abs(r_eq.price) < 0.5, f"Forward équitable hors tolérance : {r_eq.price}"
print("  [OK] Tous les forwards équitables ≈ 0")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 60)
print("  RÉSUMÉ")
print("=" * 60)
print(f"  F* sans dividende  : {F_NO_DIV:.4f}  → fwd ≈ {r_no_div.price:.4f}")
print(f"  F* avec div q={Q}  : {F_DIV:.4f}  → fwd ≈ {r_div.price:.4f}")
print(f"  Fwd(K=100) sans div: {r_atm_no.price:.4f}")
print(f"  Fwd(K=100) avec div: {r_atm_dv.price:.4f}")
print(f"  Réduction dividende: {r_atm_no.price - r_atm_dv.price:.4f}")
print("  Tous les asserts sont verts — script OK")
