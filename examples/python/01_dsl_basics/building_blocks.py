"""
Briques de base du DSL kontract.

Illustre chaque primitive élémentaire :
  zero, one, give, S (observable spot), const_ (constante observable),
  at (condition temporelle), scale (obs * one), when (@), and (+), or (.or_).

Chaque primitive est pricée pour vérifier son comportement économique :
- one(USD) @ at(T) = obligation zéro-coupon = e^{-rT}
- give inverse le signe du flux
- scale amplifie le notionnel
- and additionne deux payoffs
- or donne le choix du meilleur payoff (exercice optimal non simulé ici)
"""

import math
import kontract as k

# ---------------------------------------------------------------------------
# Paramètres communs
# ---------------------------------------------------------------------------
R = 0.05
T = 1.0
MODEL = k.GBM(s0=100.0, sigma=0.20, r=R, asset="X")
N = 40_000
SEED = 42
DISCOUNT = math.exp(-R * T)

print("=" * 60)
print("  KONTRACT — Briques de base du DSL")
print("=" * 60)

# ---------------------------------------------------------------------------
# zero() — contrat sans flux
# ---------------------------------------------------------------------------
print("\n--- zero() : contrat nul ---")
z = k.zero()
rz = z.price(MODEL, n_paths=N, seed=SEED)
print(f"  Prix zero() = {rz.price:.6f}  (attendu : 0.0)")
assert abs(rz.price) < 1e-6, "zero() doit valoir 0"
print("  [OK]")

# ---------------------------------------------------------------------------
# one(ccy) @ at(T) — obligation zéro-coupon
# ---------------------------------------------------------------------------
print("\n--- one(USD) @ at(T) : ZCB ---")
zcb = k.one(k.USD) @ k.at(T)
r_zcb = zcb.price(MODEL, n_paths=N, seed=SEED)
print(f"  Prix MC     = {r_zcb.price:.6f}")
print(f"  e^{{-rT}}     = {DISCOUNT:.6f}")
assert abs(r_zcb.price - DISCOUNT) < 0.001, "ZCB doit valoir e^{-rT}"
print("  [OK] ZCB = e^{-rT}")

# ---------------------------------------------------------------------------
# give(c) — flux inversé
# ---------------------------------------------------------------------------
print("\n--- give(one(USD) @ at(T)) : position courte ---")
short_zcb = k.give(k.one(k.USD) @ k.at(T))
r_give = short_zcb.price(MODEL, n_paths=N, seed=SEED)
print(f"  Prix MC       = {r_give.price:.6f}")
print(f"  Attendu       = {-DISCOUNT:.6f}")
assert abs(r_give.price + DISCOUNT) < 0.001, "give() doit inverser le signe"
print("  [OK] give() inverse le signe")

# ---------------------------------------------------------------------------
# S("X") — observable spot
# ---------------------------------------------------------------------------
print("\n--- S('X') : observable spot ---")
spot_contract = (k.S("X") * k.one(k.USD)) @ k.at(T)
r_spot = spot_contract.price(MODEL, n_paths=N, seed=SEED)
# Prépaid forward = S0 (sans dividende)
print(f"  Prix MC prépaid forward = {r_spot.price:.4f}")
print(f"  Attendu ≈ S0 = 100.0")
assert abs(r_spot.price - 100.0) < 1.5, "Prépaid forward ≈ S0"
print("  [OK] Prépaid fwd ≈ S0")

# ---------------------------------------------------------------------------
# const_(x) — observable constant
# ---------------------------------------------------------------------------
print("\n--- const_(5.0) : observable constante ---")
fixed_5 = (k.const_(5.0) * k.one(k.USD)) @ k.at(T)
r_fixed = fixed_5.price(MODEL, n_paths=N, seed=SEED)
print(f"  Prix MC = {r_fixed.price:.4f}  (attendu : 5 * {DISCOUNT:.4f} = {5*DISCOUNT:.4f})")
assert abs(r_fixed.price - 5.0 * DISCOUNT) < 0.005
print("  [OK]")

# ---------------------------------------------------------------------------
# scale(obs, one) = obs * one — notionnel variable
# ---------------------------------------------------------------------------
print("\n--- scale : (2.5 * S('X')) * one(USD) @ at(T) ---")
scaled = (2.5 * k.S("X") * k.one(k.USD)) @ k.at(T)
r_scaled = scaled.price(MODEL, n_paths=N, seed=SEED)
print(f"  Prix MC = {r_scaled.price:.4f}  (attendu ≈ 2.5 * 100 = 250)")
assert abs(r_scaled.price - 250.0) < 3.0
print("  [OK] scale ≈ notionnel * S0")

# ---------------------------------------------------------------------------
# and (+) — portefeuille de deux contrats
# ---------------------------------------------------------------------------
print("\n--- and (+) : ZCB + scaled ---")
portfolio = (k.one(k.USD) @ k.at(T)) + fixed_5
r_and = portfolio.price(MODEL, n_paths=N, seed=SEED)
expected_and = DISCOUNT + 5.0 * DISCOUNT
print(f"  Prix MC = {r_and.price:.4f}  (attendu : {expected_and:.4f})")
assert abs(r_and.price - expected_and) < 0.01
print("  [OK] and = somme des parties")

# ---------------------------------------------------------------------------
# or (.or_) — choix du meilleur payoff (non encore priceable, J17/LSM)
# ---------------------------------------------------------------------------
print("\n--- or (.or_) : max(1 USD, 0.5 USD) @ at(T) ---")
# Le détenteur choisit de recevoir 1 USD ou 0.5 USD — il choisira toujours 1 USD.
# Note : le pricing du choix optimal nécessite Least-Squares MC (jalon J17).
# On illustre ici la construction DSL uniquement.
c_big = (k.const_(1.0) * k.one(k.USD)) @ k.at(T)
c_small = (k.const_(0.5) * k.one(k.USD)) @ k.at(T)
best = c_big.or_(c_small)
print(f"  Repr : {best!r}")
print("  [INFO] or() non priceable dans le MVP (LSM requis, jalon J17)")
print("  [OK] or() construit sans erreur ; exercice optimal non simulé ici")
# Borne inférieure : la valeur est au moins celle du meilleur flux sans choix
# On vérifie par les deux branches séparément
r_big   = c_big.price(MODEL, n_paths=N, seed=SEED)
r_small = c_small.price(MODEL, n_paths=N, seed=SEED)
assert r_big.price > r_small.price, "La branche de 1 USD doit dominer celle de 0.5 USD"
print(f"  Borne inf : max({r_big.price:.4f}, {r_small.price:.4f}) = {max(r_big.price, r_small.price):.4f}")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 60)
print("  RÉSUMÉ des primitives")
print("=" * 60)
print(f"  zero()                  : {rz.price:.4f}")
print(f"  one(USD) @ at(1)        : {r_zcb.price:.4f}  [e^{{-r}}={DISCOUNT:.4f}]")
print(f"  give(ZCB)               : {r_give.price:.4f}")
print(f"  const_(5)*one @ at(1)  : {r_fixed.price:.4f}")
print(f"  S('X')*one @ at(1)     : {r_spot.price:.4f}  [≈S0=100]")
print(f"  ZCB + fixed_5           : {r_and.price:.4f}  [and]")
print(f"  big.or_(small)          : borne inf = {max(r_big.price, r_small.price):.4f}  [or, LSM J17]")
print("  Tous les asserts sont verts — script OK")
