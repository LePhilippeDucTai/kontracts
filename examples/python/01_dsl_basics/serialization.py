"""
Sérialisation JSON des contrats kontract.

Démontre la persistance et le round-trip JSON :
1. Construire un contrat à barrière (call up-and-out)
2. Sérialiser en JSON via c.to_json()
3. Désérialiser via k.Contract.from_json(s)
4. Vérifier que le prix du round-trip est identique (même seed, même modèle)

Cas d'usage : sauvegarder un portefeuille de contrats en base de données,
le transférer entre services, ou l'auditer.
"""

import math
import json
import kontract as k

MODEL = k.GBM(s0=100.0, sigma=0.20, r=0.05, asset="X")
N = 60_000
SEED = 42
T = 1.0

print("=" * 60)
print("  KONTRACT — Sérialisation JSON")
print("=" * 60)

# ---------------------------------------------------------------------------
# 1. Contrat simple : ZCB
# ---------------------------------------------------------------------------
print("\n--- ZCB : one(USD) @ at(1) ---")
zcb = k.one(k.USD) @ k.at(T)
js_zcb = zcb.to_json()
print(f"  JSON : {js_zcb}")
zcb2 = k.Contract.from_json(js_zcb)
r1 = zcb.price(MODEL, n_paths=N, seed=SEED)
r2 = zcb2.price(MODEL, n_paths=N, seed=SEED)
print(f"  Prix original    : {r1.price:.8f}")
print(f"  Prix round-trip  : {r2.price:.8f}")
assert abs(r1.price - r2.price) < 1e-10, "Round-trip ZCB doit être identique"
print("  [OK] Round-trip ZCB exact")

# ---------------------------------------------------------------------------
# 2. Contrat à barrière : call up-and-out
# ---------------------------------------------------------------------------
print("\n--- Call up-and-out (H=140) ---")
vanilla = ((k.S("X") - 100.0).clip(0.0) * k.one(k.USD)) @ k.at(T)
ko_call = vanilla.until(k.S("X") >= 140.0)

js_ko = ko_call.to_json()
print(f"  JSON (début) : {js_ko[:120]}...")

# Vérifier que c'est du JSON valide
parsed = json.loads(js_ko)
print(f"  Type JSON racine : {list(parsed.keys()) if isinstance(parsed, dict) else type(parsed).__name__}")

# Désérialisation
ko_call2 = k.Contract.from_json(js_ko)
print("  Désérialisation : OK")

# Pricing identique (même seed → même résultat déterministe)
r_orig = ko_call.price(MODEL, n_paths=N, seed=SEED, steps_per_year=50)
r_rt   = ko_call2.price(MODEL, n_paths=N, seed=SEED, steps_per_year=50)
print(f"  Prix original    : {r_orig.price:.8f}")
print(f"  Prix round-trip  : {r_rt.price:.8f}")
assert abs(r_orig.price - r_rt.price) < 1e-10, "Round-trip KO call doit être identique"
print("  [OK] Round-trip KO call exact")

# ---------------------------------------------------------------------------
# 3. Contrat complexe : portefeuille call spread + ZCB
# ---------------------------------------------------------------------------
print("\n--- Portefeuille complexe : bull call spread + ZCB ---")
call_90 = ((k.S("X") - 90.0).clip(0.0) * k.one(k.USD)) @ k.at(T)
call_110 = ((k.S("X") - 110.0).clip(0.0) * k.one(k.USD)) @ k.at(T)
spread = call_90 + (-call_110)
portfolio = spread + (k.const_(5.0) * k.one(k.USD) @ k.at(T))

js_port = portfolio.to_json()
print(f"  Taille JSON (octets) : {len(js_port)}")

portfolio2 = k.Contract.from_json(js_port)
r_p1 = portfolio.price(MODEL, n_paths=N, seed=SEED)
r_p2 = portfolio2.price(MODEL, n_paths=N, seed=SEED)
print(f"  Prix original    : {r_p1.price:.8f}")
print(f"  Prix round-trip  : {r_p2.price:.8f}")
assert abs(r_p1.price - r_p2.price) < 1e-10, "Round-trip portfolio doit être identique"
print("  [OK] Round-trip portefeuille exact")

# ---------------------------------------------------------------------------
# 4. Inspection du JSON produit
# ---------------------------------------------------------------------------
print("\n--- Inspection du JSON du call KO ---")
js_pretty = json.dumps(json.loads(js_ko), indent=2)
lines = js_pretty.split("\n")
preview = "\n".join(lines[:20])
print(preview)
if len(lines) > 20:
    print(f"  ... ({len(lines) - 20} lignes supplémentaires)")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 60)
print("  RÉSUMÉ")
print("=" * 60)
print(f"  ZCB prix              : {r1.price:.6f}")
print(f"  KO call prix          : {r_orig.price:.6f}")
print(f"  Portefeuille prix     : {r_p1.price:.6f}")
print("  Tous les round-trips sont exacts bit-à-bit")
print("  Tous les asserts sont verts — script OK")
