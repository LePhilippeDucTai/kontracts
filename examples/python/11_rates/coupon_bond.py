"""
Obligation à coupons sous taux stochastiques (Vasicek).

Une obligation à coupons est construite comme une somme de ZCB via l'opérateur
+ du DSL : c1 + c2 + ... (portefeuille).  Le prix Monte-Carlo est comparé à
la somme analytique des facteurs d'actualisation rm.discount_bond0(t_i).

Deux obligations illustrées :
  - Obligation 3 ans, coupon annuel 4 %, pair 100
  - Obligation 5 ans, coupon semestriel 3 %, pair 100
"""

import kontract as k

# ---------------------------------------------------------------------------
# Paramètres du modèle
# ---------------------------------------------------------------------------
RM = k.vasicek(r0=0.03, a=0.6, b=0.05, sigma=0.015)
N_PATHS = 150_000
SEED = 42
STEPS = 100

# ---------------------------------------------------------------------------
# Construction d'une obligation à coupons via le DSL
# ---------------------------------------------------------------------------

def coupon_bond_contract(coupon_dates: list[float], coupon: float, principal: float):
    """
    Construit un contrat d'obligation à coupons.

    Chaque flux est un ZCB mis à l'échelle :
      coupon * ZCB(t_i) pour les coupons intermédiaires
      (coupon + principal) * ZCB(t_N) pour le dernier flux.

    Retourne le contrat somme (portfolio DSL via +).
    """
    flows = [
        k.const_(coupon) * k.zero_coupon_bond(k.USD, t)
        for t in coupon_dates[:-1]
    ]
    last = k.const_(coupon + principal) * k.zero_coupon_bond(k.USD, coupon_dates[-1])
    return sum(flows, last) if flows else last


def analytic_coupon_bond(rm, coupon_dates: list[float], coupon: float, principal: float) -> float:
    """Prix analytique comme somme pondérée des discount_bond0."""
    return (
        sum(coupon * rm.discount_bond0(t) for t in coupon_dates[:-1])
        + (coupon + principal) * rm.discount_bond0(coupon_dates[-1])
    )


# ---------------------------------------------------------------------------
# Affichage
# ---------------------------------------------------------------------------

def benchmark_bond(
    label: str,
    coupon_dates: list[float],
    coupon: float,
    principal: float,
) -> None:
    contract = coupon_bond_contract(coupon_dates, coupon, principal)
    mc_res = contract.price_under_rates(RM, n_paths=N_PATHS, seed=SEED, steps_per_year=STEPS)
    analytic = analytic_coupon_bond(RM, coupon_dates, coupon, principal)
    rel_err = abs(mc_res.price - analytic) / analytic

    print(f"\n  {label}")
    print(f"    Dates de coupon    : {coupon_dates}")
    print(f"    Coupon / Principal : {coupon} / {principal}")
    print(f"    Prix MC            : {mc_res.price:.4f} ± {mc_res.std_error:.4f}")
    print(f"    Prix analytique    : {analytic:.4f}")
    print(f"    Erreur relative    : {rel_err:.5%}")
    assert rel_err < 0.01, f"Erreur relative trop grande : {rel_err:.4%}"
    print(f"    [OK] Erreur < 1 %")


# ---------------------------------------------------------------------------
# Deux obligations
# ---------------------------------------------------------------------------
print("=" * 60)
print("  OBLIGATION À COUPONS — Vasicek — MC vs Analytique")
print("=" * 60)
print(f"  Vasicek : r0=3 %, a=0.6, b=5 %, σ=1.5 %")
print(f"  N chemins = {N_PATHS:,}  —  seed = {SEED}")

# Obligation 3 ans, coupon annuel 4 %, pair 100
benchmark_bond(
    "Obligation 3 ans — coupon annuel 4 %, pair 100",
    coupon_dates=[1.0, 2.0, 3.0],
    coupon=4.0,
    principal=100.0,
)

# Obligation 5 ans, coupon semestriel 3 %, pair 100
# Coupons semestriels = 1.5 par semestre (3 % * 100 / 2)
benchmark_bond(
    "Obligation 5 ans — coupon semestriel 3 %, pair 100",
    coupon_dates=[0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0],
    coupon=1.5,
    principal=100.0,
)

print("\n" + "=" * 60)
print("  RÉSUMÉ")
print("=" * 60)
print("  Portefeuille DSL (+) de ZCB ≈ prix analytique à < 1 %.")
print("  Tous les asserts sont verts — script OK")
