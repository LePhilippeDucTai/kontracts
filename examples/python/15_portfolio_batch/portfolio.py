"""
Livre de contrats — Pricing par lot, Greeks, sérialisation JSON.

Cinq contrats illustrés :
  1. Call européen ATM
  2. Put européen ATM
  3. Straddle ATM  (call + put combinés)
  4. Call up-and-out (barrière H=150)
  5. ZCB 1 an (obligation zéro-coupon, delta ≈ 0)

Pour chaque contrat :
  - Prix ± std_error
  - Delta (via .greeks, sauf ZCB dont delta = 0 par construction)

Agrégats du livre :
  - PV totale  = Σ prix
  - Delta total = Σ deltas

Sérialisation :
  - to_json() → chaîne JSON par contrat
  - Contract.from_json() → rechargement
  - Vérification : prix rechargé == prix original
"""

import json
import kontract as k

# ---------------------------------------------------------------------------
# Modèle et paramètres
# ---------------------------------------------------------------------------
S0    = 100.0
K     = 100.0
T     = 1.0
SIGMA = 0.20
R     = 0.05

MODEL = k.GBM(s0=S0, sigma=SIGMA, r=R, asset="X")

N_PATHS       = 100_000
N_PATHS_GREEK = 200_000
SEED          = 42

# ---------------------------------------------------------------------------
# Construction du livre
# ---------------------------------------------------------------------------
BOOK: list[tuple[str, k.Contract]] = [
    ("Call européen ATM",         k.european_call("X", K, T, k.USD)),
    ("Put européen ATM",          k.european_put("X", K, T, k.USD)),
    ("Straddle ATM",              k.straddle("X", K, T, k.USD)),
    ("Call up-and-out (H=150)",   k.up_and_out_call("X", K, 150.0, T, k.USD)),
    ("ZCB 1 an (delta=0)",        k.zero_coupon_bond(k.USD, T)),
]

# ---------------------------------------------------------------------------
# Pricing + Greeks
# ---------------------------------------------------------------------------
results: list[tuple[str, k.PriceResult, float]] = []

print("=" * 72)
print("  LIVRE DE CONTRATS — Prix, Greeks, Sérialisation")
print("=" * 72)
print(f"  GBM : S0={S0}, σ={SIGMA}, r={R}  |  N chemins={N_PATHS:,}  seed={SEED}")
print(f"\n  {'Contrat':<30}  {'Prix':>8}  {'Std.Err.':>8}  {'Delta':>8}")
print(f"  {'-'*62}")

for label, contract in BOOK:
    # Prix Monte-Carlo
    res = contract.price(MODEL, n_paths=N_PATHS, seed=SEED, steps_per_year=50)
    # Delta via Greeks (GBM)
    greeks = contract.greeks(MODEL, n_paths=N_PATHS_GREEK, seed=SEED)
    delta  = greeks.delta

    results.append((label, res, delta))
    print(f"  {label:<30}  {res.price:>8.4f}  {res.std_error:>8.4f}  {delta:>8.4f}")

# ---------------------------------------------------------------------------
# Agrégats
# ---------------------------------------------------------------------------
total_pv    = sum(r.price for _, r, _ in results)
total_delta = sum(d for _, _, d in results)

print(f"\n  {'TOTAL':<30}  {total_pv:>8.4f}  {'':>8}  {total_delta:>8.4f}")
print(f"  (NB : delta total inclut le ZCB dont delta=0 par construction)")

# ---------------------------------------------------------------------------
# Sérialisation JSON — aller-retour
# ---------------------------------------------------------------------------
print(f"\n{'='*72}")
print("  SÉRIALISATION JSON — aller-retour")
print(f"{'='*72}")

book_json: list[str] = [contract.to_json() for _, contract in BOOK]

# Afficher un extrait du JSON du premier contrat
print(f"\n  Extrait JSON du call européen (70 premiers chars) :")
print(f"  {book_json[0][:70]}…")

# Rechargement
book_reloaded: list[k.Contract] = [k.Contract.from_json(js) for js in book_json]

# Vérification : prix rechargé == prix original
print(f"\n  {'Contrat':<30}  {'Prix orig.':>10}  {'Prix reload.':>12}  {'Match':>6}")
print(f"  {'-'*65}")

all_match = True
for (label, orig_contract, _), reloaded_contract in zip(
    [(l, c, None) for l, c in BOOK], book_reloaded
):
    r_orig     = orig_contract.price(MODEL, n_paths=N_PATHS, seed=SEED, steps_per_year=50)
    r_reloaded = reloaded_contract.price(MODEL, n_paths=N_PATHS, seed=SEED, steps_per_year=50)
    match      = abs(r_orig.price - r_reloaded.price) < 1e-10
    all_match  = all_match and match
    tag        = "OK" if match else "FAIL"
    print(
        f"  {label:<30}  {r_orig.price:>10.6f}  {r_reloaded.price:>12.6f}  {tag:>6}"
    )

assert all_match, "Des contrats rechargés donnent un prix différent de l'original"
print(f"\n  [OK] Tous les prix rechargés == prix originaux (à 1e-10 près)")

# Sauvegarder le livre complet en JSON (liste de chaînes JSON)
livre_json = json.dumps(book_json, ensure_ascii=False, indent=2)
print(f"\n  Livre JSON sérialisé : {len(livre_json)} caractères, {len(book_json)} contrats")

# ---------------------------------------------------------------------------
# Résumé du portefeuille
# ---------------------------------------------------------------------------
print(f"\n{'='*72}")
print("  RÉCAPITULATIF DU PORTEFEUILLE")
print(f"{'='*72}")
for label, res, delta in results:
    print(f"  {label:<30}  PV={res.price:>8.4f}  Δ={delta:>8.4f}")
print(f"  {'-'*62}")
print(f"  {'PV totale':<30}  {total_pv:>8.4f}")
print(f"  {'Delta total':<30}  {total_delta:>8.4f}")
print(f"\n  N chemins pricing  = {N_PATHS:,}")
print(f"  N chemins Greeks   = {N_PATHS_GREEK:,}")
print(f"  Sérialisation JSON : OK")
print("  Tous les asserts sont verts — script OK")
