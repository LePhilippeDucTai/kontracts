"""
Obligation zéro-coupon sous taux stochastiques.

Compare k.zero_coupon_bond(k.USD, T).price_under_rates(model) au prix
analytique rm.discount_bond0(T) pour les modèles Vasicek et Hull-White,
sur plusieurs maturités.  La propriété centrale : le prix Monte-Carlo doit
reproduire le prix analytique (formule de Bond fermée) à mieux que 1 %.
"""

import kontract as k

# ---------------------------------------------------------------------------
# Paramètres
# ---------------------------------------------------------------------------
MATURITIES = [0.5, 1.0, 2.0, 3.0, 5.0]
N_PATHS = 150_000
SEED = 42
STEPS = 100

# Vasicek : r0=3 %, mean-reversion a=0.6, long-terme b=5 %, vol σ=1.5 %
VASICEK = k.vasicek(r0=0.03, a=0.6, b=0.05, sigma=0.015)

# Hull-White : r0=3 %, mean-reversion a=0.6, vol σ=1 %
HULL_WHITE = k.hull_white(r0=0.03, a=0.6, sigma=0.01)

# ---------------------------------------------------------------------------
# Fonction d'affichage et de vérification
# ---------------------------------------------------------------------------

def benchmark_model(model, label: str) -> None:
    print(f"\n{'='*60}")
    print(f"  Modèle : {label}")
    print(f"{'='*60}")
    print(f"  {'T':>4}  {'MC':>10}  {'Analytique':>12}  {'Err. rel.':>10}  {'[OK]':>5}")
    print(f"  {'-'*55}")

    for T in MATURITIES:
        zc = k.zero_coupon_bond(k.USD, T)
        mc_res = zc.price_under_rates(
            model,
            n_paths=N_PATHS,
            seed=SEED,
            steps_per_year=STEPS,
        )
        analytic = model.discount_bond0(T)
        rel_err = abs(mc_res.price - analytic) / analytic

        ok = "OK" if rel_err < 0.01 else "FAIL"
        print(
            f"  {T:>4.1f}  {mc_res.price:>10.6f}  {analytic:>12.6f}  "
            f"{rel_err:>9.4%}  {ok:>5}"
        )
        assert rel_err < 0.01, (
            f"{label} T={T}: erreur relative {rel_err:.4%} ≥ 1 %"
        )

    print(f"\n  [OK] Toutes les maturités : erreur relative < 1 %")


# ---------------------------------------------------------------------------
# Résultat
# ---------------------------------------------------------------------------
print("=" * 60)
print("  ZCB Monte-Carlo vs Analytique — taux stochastiques")
print("=" * 60)

benchmark_model(VASICEK, "Vasicek (r0=3%, a=0.6, b=5%, σ=1.5%)")
benchmark_model(HULL_WHITE, "Hull-White (r0=3%, a=0.6, σ=1%)")

print("\n" + "=" * 60)
print("  RÉSUMÉ")
print("=" * 60)
print("  Vasicek et Hull-White : MC ≈ analytique à < 1 %.")
print("  Maturités testées :", MATURITIES)
print("  N chemins =", N_PATHS, " — seed =", SEED)
print("  Tous les asserts sont verts — script OK")
