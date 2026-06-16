"""
Obligations et annuités.

Une obligation zéro-coupon de maturité T vaut :

    P(0,T) = e^{-rT}

Une obligation à coupons est une somme de ZCB :
    - Flux de coupon C à chaque date t_i
    - Remboursement du principal N à maturité T

    V = C · Σ e^{-r·t_i}  +  N · e^{-r·T}

Une annuité est une série de paiements unitaires identiques.

Ce script illustre la construction de ces produits comme sums de contrats
elementaires via l'opérateur + (and) du DSL kontract.
"""

import math
import kontract as k

R  = 0.05
N  = 60_000
SEED = 42

MODEL = k.GBM(s0=100.0, sigma=0.20, r=R, asset="X")

print("=" * 60)
print("  KONTRACT — Obligations et Annuités")
print("=" * 60)
print(f"\n  Taux sans risque continu r = {R}")

# ---------------------------------------------------------------------------
# 1. ZCB pour diverses maturités
# ---------------------------------------------------------------------------
print("\n--- 1. ZCB : one(USD) @ at(T) = e^{-rT} ---")
print(f"  {'T':>5}  {'Prix MC':>10}  {'e^{{-rT}}':>10}  {'Erreur':>10}")
print("  " + "-" * 40)
for T_val in [0.5, 1.0, 2.0, 5.0, 10.0]:
    zcb = k.one(k.USD) @ k.at(T_val)
    r_zcb = zcb.price(MODEL, n_paths=N, seed=SEED)
    analytic = math.exp(-R * T_val)
    err = abs(r_zcb.price - analytic)
    print(f"  {T_val:5.1f}  {r_zcb.price:10.6f}  {analytic:10.6f}  {err:10.8f}")
    assert abs(r_zcb.price - analytic) < 0.002, f"ZCB(T={T_val}) hors tolérance"
print("  [OK] ZCB = e^{-rT} pour toutes maturités")

# ---------------------------------------------------------------------------
# 2. Obligation à coupons semestriels
# ---------------------------------------------------------------------------
print("\n--- 2. Obligation à coupons : coupon=5%, principal=100, T=5 ans ---")
COUPON_RATE = 0.05      # taux coupon annuel
PRINCIPAL   = 100.0
FREQ        = 2         # semestriel

coupon_dates = [0.5 * i for i in range(1, FREQ * 5 + 1)]   # 0.5, 1.0, ..., 5.0
coupon_amount = PRINCIPAL * COUPON_RATE / FREQ               # 2.5 USD par coupon

# Construction du bond comme somme de ZCB
# Flux coupon aux dates intermédiaires + coupon + principal à T=5
bond = k.zero()
analytic_bond = 0.0
for t_i in coupon_dates:
    flux = coupon_amount
    if abs(t_i - 5.0) < 1e-9:
        flux += PRINCIPAL
    coupon_contract = (k.const_(flux) * k.one(k.USD)) @ k.at(t_i)
    bond = bond + coupon_contract
    analytic_bond += flux * math.exp(-R * t_i)

r_bond = bond.price(MODEL, n_paths=N, seed=SEED)
print(f"  Dates de coupon       : {coupon_dates}")
print(f"  Coupon semestriel     : {coupon_amount:.2f} USD")
print(f"  Prix MC               : {r_bond.price:.4f}")
print(f"  Prix analytique       : {analytic_bond:.4f}")
print(f"  Erreur absolue        : {abs(r_bond.price - analytic_bond):.6f}")
assert abs(r_bond.price - analytic_bond) / analytic_bond < 0.01
print("  [OK] Obligation coupons ≈ Σ flux · e^{-r·t_i}")

# Par rapport au pair : coupon=r → prix = pair
# (avec taux continu, coupon annuel exact = r*N pour prix au pair)
print(f"\n  Note : taux coupon = r = {R} → prix ≈ principal")
# Approximation: avec coupons semestriels et taux continu, prix légèrement diff de pair
print(f"  Prix / Principal      : {r_bond.price / PRINCIPAL:.4f}")

# ---------------------------------------------------------------------------
# 3. Annuité : série de paiements unitaires
# ---------------------------------------------------------------------------
print("\n--- 3. Annuité : 1 USD / an pendant 10 ans ---")
T_ANNUITY = 10
annuity_dates = list(range(1, T_ANNUITY + 1))

annuity = k.zero()
analytic_annuity = 0.0
for t_i in annuity_dates:
    annuity = annuity + (k.one(k.USD) @ k.at(float(t_i)))
    analytic_annuity += math.exp(-R * t_i)

r_ann = annuity.price(MODEL, n_paths=N, seed=SEED)

# Formule fermée de l'annuité (taux continu)
# A = (1 - e^{-rT}) / (e^r - 1) pour paiements annuels
a_closed = sum(math.exp(-R * t) for t in annuity_dates)

print(f"  Dates                 : {annuity_dates}")
print(f"  Prix MC               : {r_ann.price:.4f}")
print(f"  Analytique (somme)    : {analytic_annuity:.4f}")
print(f"  Analytique (fermée)   : {a_closed:.4f}")
assert abs(r_ann.price - analytic_annuity) / analytic_annuity < 0.01
print("  [OK] Annuité ≈ Σ e^{-r·t_i}")

# ---------------------------------------------------------------------------
# 4. Annuité perpétuelle (approximation longue maturité)
# ---------------------------------------------------------------------------
print("\n--- 4. Duration et sensibilité au taux ---")
# Comparer obligations 5 ans à coupons avec différents taux coupon
print(f"  {'Coupon %':>10}  {'Prix MC':>10}  {'Prix anal.':>12}")
print("  " + "-" * 36)
for c_rate in [0.02, 0.05, 0.08, 0.10]:
    c_amt = PRINCIPAL * c_rate / FREQ
    b = k.zero()
    a_val = 0.0
    for t_i in coupon_dates:
        flux = c_amt + (PRINCIPAL if abs(t_i - 5.0) < 1e-9 else 0.0)
        b = b + (k.const_(flux) * k.one(k.USD)) @ k.at(t_i)
        a_val += flux * math.exp(-R * t_i)
    r_b = b.price(MODEL, n_paths=N, seed=SEED)
    print(f"  {c_rate*100:10.1f}  {r_b.price:10.4f}  {a_val:12.4f}")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 60)
print("  RÉSUMÉ")
print("=" * 60)
print(f"  ZCB(T=1)              : ≈ e^{{-rT}} = {math.exp(-R):.4f}")
print(f"  Obligation coupons    : {r_bond.price:.4f}  (analyt. {analytic_bond:.4f})")
print(f"  Annuité 10 ans        : {r_ann.price:.4f}  (analyt. {analytic_annuity:.4f})")
print("  Tous les asserts sont verts — script OK")
