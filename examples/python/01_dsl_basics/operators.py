"""
Opérateurs du DSL kontract.

Montre la syntaxe des opérateurs disponibles :
- @ (when) : conditionner un contrat sur une condition temporelle ou de prix
- * (scale) : multiplier un observable par one(ccy)
- + (and)   : additionner deux contrats (portefeuille)
- unaire -  : give — inverser tous les flux
- ~ (not)   : négation d'une Condition
- & (and)   : conjonction de deux Conditions
- | (or)    : disjonction de deux Conditions
- >=, >, <=, < : comparaisons produisant des Condition

Cas d'usage pédagogique : corridor (90 <= S <= 110) via & sur des Condition.
"""

import math
import kontract as k

MODEL = k.GBM(s0=100.0, sigma=0.20, r=0.05, asset="X")
N = 40_000
SEED = 42
T = 1.0

print("=" * 60)
print("  KONTRACT — Opérateurs du DSL")
print("=" * 60)

# ---------------------------------------------------------------------------
# @ (when) — conditionnement temporel
# ---------------------------------------------------------------------------
print("\n--- @ (when) : paiement à T=1 ---")
c_when = k.one(k.USD) @ k.at(T)
print(f"  Repr  : {c_when!r}")
r = c_when.price(MODEL, n_paths=N, seed=SEED)
print(f"  Prix  : {r.price:.4f}  (attendu e^{{-0.05}}={math.exp(-0.05):.4f})")

# ---------------------------------------------------------------------------
# * (scale) — notionnel via observable
# ---------------------------------------------------------------------------
print("\n--- * (scale) : observable * one(ccy) ---")
obs_triple = 3.0 * k.S("X")          # Observable * float
contract_scaled = (obs_triple * k.one(k.USD)) @ k.at(T)
print(f"  Repr observable 3*S : {obs_triple!r}")
r_sc = contract_scaled.price(MODEL, n_paths=N, seed=SEED)
print(f"  Prix 3*S @ T=1      : {r_sc.price:.4f}  (attendu ≈ 3*100=300)")

# ---------------------------------------------------------------------------
# + (and) — portefeuille
# ---------------------------------------------------------------------------
print("\n--- + (and) : portefeuille de contrats ---")
c1 = k.one(k.USD) @ k.at(T)
c2 = (k.const_(2.0) * k.one(k.USD)) @ k.at(T)
portfolio = c1 + c2
print(f"  Repr  : {portfolio!r}")
r_p = portfolio.price(MODEL, n_paths=N, seed=SEED)
disc = math.exp(-0.05 * T)
print(f"  Prix  : {r_p.price:.4f}  (attendu 3 * {disc:.4f} = {3*disc:.4f})")
assert abs(r_p.price - 3 * disc) < 0.005

# ---------------------------------------------------------------------------
# unaire - (give) — position courte
# ---------------------------------------------------------------------------
print("\n--- unaire - (give) : position courte ---")
short_c1 = -c1
r_short = short_c1.price(MODEL, n_paths=N, seed=SEED)
print(f"  Prix  : {r_short.price:.4f}  (attendu {-disc:.4f})")
assert abs(r_short.price + disc) < 0.002

# ---------------------------------------------------------------------------
# Conditions : >=, >, <=, <
# ---------------------------------------------------------------------------
print("\n--- Comparaisons -> Condition ---")
cond_up   = k.S("X") >= 110.0
cond_down = k.S("X") <= 90.0
cond_lt   = k.S("X") <  100.0
cond_gt   = k.S("X") >  100.0
print(f"  S >= 110 : {cond_up!r}")
print(f"  S <= 90  : {cond_down!r}")
print(f"  S < 100  : {cond_lt!r}")
print(f"  S > 100  : {cond_gt!r}")

# ---------------------------------------------------------------------------
# & — conjonction (corridor)
# ---------------------------------------------------------------------------
print("\n--- & (and Condition) : corridor 90 <= S <= 110 ---")
corridor = (k.S("X") >= 90.0) & (k.S("X") <= 110.0)
print(f"  Repr corridor : {corridor!r}")
# Contrat : reçoit 1 USD si S dans le corridor à T=1
# On utilise un when manuel via un contrat scale conditionnel
# Note: when + condition de prix → l'horizon est fixé par le at(T) dans l'arbre
c_corridor = (k.const_(1.0) * k.one(k.USD)) @ k.at(T)
# En pratique on l'associe avec .until ou dans une option digitale construite à la main
# Ici on montre le type Condition et sa combinaison
print("  Type de corridor :", type(corridor).__name__)
assert isinstance(corridor, k.Condition)
print("  [OK] & produit bien une Condition")

# ---------------------------------------------------------------------------
# | — disjonction
# ---------------------------------------------------------------------------
print("\n--- | (or Condition) : S < 90 | S > 110 ---")
outside_corridor = (k.S("X") < 90.0) | (k.S("X") > 110.0)
print(f"  Repr : {outside_corridor!r}")
assert isinstance(outside_corridor, k.Condition)
print("  [OK] | produit bien une Condition")

# ---------------------------------------------------------------------------
# ~ — négation
# ---------------------------------------------------------------------------
print("\n--- ~ (not Condition) : ~corridor = hors corridor ---")
not_corridor = ~corridor
print(f"  Repr : {not_corridor!r}")
assert isinstance(not_corridor, k.Condition)
print("  [OK] ~ produit bien une Condition")

# ---------------------------------------------------------------------------
# Exemple pricé : call conditionnel (tunnel option)
# Reçoit le payoff max(S-100,0) seulement si S démarre dans le corridor
# Ici on construit un call vanille et on l'annule par give si S >= 110 à expiry
# ---------------------------------------------------------------------------
print("\n--- Exemple pricé : call vs tunnel (knock-out corridor à T) ---")
vanilla_call = ((k.S("X") - 100.0).clip(0.0) * k.one(k.USD)) @ k.at(T)
r_vanilla = vanilla_call.price(MODEL, n_paths=N, seed=SEED)

# Call tunnel : KO si S > 130 (jusqu'à maturité)
tunnel_call = vanilla_call.until(k.S("X") >= 130.0)
r_tunnel = tunnel_call.price(MODEL, n_paths=N, seed=SEED, steps_per_year=50)

print(f"  Call vanille   : {r_vanilla.price:.4f}")
print(f"  Call KO (H=130): {r_tunnel.price:.4f}")
assert r_tunnel.price < r_vanilla.price
print("  [OK] KO < vanille (barrière absorbe de la valeur)")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 60)
print("  RÉSUMÉ des opérateurs")
print("=" * 60)
print("  Opérateur  |  Entrée          |  Sortie")
print("  -----------|------------------|----------")
print("  @          |  Contract,Cond   |  Contract  (when)")
print("  *          |  Observable,one  |  Contract  (scale)")
print("  +          |  Contract,Cntrt  |  Contract  (and)")
print("  unaire -   |  Contract        |  Contract  (give)")
print("  >=,<=,>,<  |  Observable,flt  |  Condition")
print("  &          |  Cond, Cond      |  Condition (et)")
print("  |          |  Cond, Cond      |  Condition (ou)")
print("  ~          |  Condition       |  Condition (non)")
print("  Tous les asserts sont verts — script OK")
