"""Note a rachat anticipe (Autocallable).

PROFIL DE PAYOFF
----------------
Produit a observation continue (ici sur tout l'intervalle [0,T]). A la
PREMIERE date ou le sous-jacent franchit la barriere B (S >= B), la note est
rappelee ("autocall") et l'investisseur recoit immediatement le notional plus
un coupon : N + coupon. Si la barriere n'est JAMAIS atteinte avant l'echeance T,
l'investisseur recupere simplement le notional N a maturite.

RATIONALE INVESTISSEUR
----------------------
L'investisseur vend implicitement de l'optionalite (il plafonne son gain au
coupon) en echange d'un rendement attractif si le sous-jacent stagne ou monte
moderement. Le capital est ici protege a 100 % (N a T dans le pire cas), ce qui
en fait une variante "capital garanti" de l'autocall.

CONSTRUCTION DSL
----------------
    auto  = (N+coupon) one(USD)  .anytime(S>=B)        # rappel anticipe
    floor = (N one(USD) @ at(T)) .until(S>=B)          # sinon notional a T
    note  = auto + floor

Le sibling `@ at(T)` de la branche `.until` fournit l'HORIZON T a tout l'arbre :
sans lui, les conditions de barriere ne creeraient aucun horizon et le prix
serait exactement 0 (voir CRITICAL HORIZON RULE).
"""

import math

import kontract as k

S0 = 100.0
SIGMA = 0.2
R = 0.05
T = 1.0
N = 100.0
COUPON = 8.0
DISC = math.exp(-R * T)

model = k.GBM(s0=S0, sigma=SIGMA, r=R, asset="X")


def build_autocallable(barrier: float):
    """Construit l'autocallable pour une barriere donnee."""
    autocall = (k.const_(N + COUPON) * k.one(k.USD)).anytime(k.S("X") >= barrier)
    redemption = ((k.const_(N) * k.one(k.USD)) @ k.at(T)).until(k.S("X") >= barrier)
    return autocall + redemption


print("=" * 64)
print("NOTE A RACHAT ANTICIPE (AUTOCALLABLE)")
print("=" * 64)
print(f"  Notional N        = {N:.2f}")
print(f"  Coupon            = {COUPON:.2f}")
print(f"  Maturite T        = {T:.2f}  (discount e^-rT = {DISC:.4f})")
print(f"  Cap actualise     = (N+coupon)*disc = {(N + COUPON) * DISC:.4f}")
print()

barrier = 120.0
note = build_autocallable(barrier)
res = note.price(model, n_paths=120000, seed=42, steps_per_year=100)

print(f"Barriere B = {barrier:.1f}")
print(f"  PV  = {res.price:.4f}  +/- {res.std_error:.4f}")
print(f"  IC95 = [{res.ci95_low:.4f}, {res.ci95_high:.4f}]")
print()

# Effet de la barriere : plus la barriere est basse, plus le rappel est probable
# et tot, donc plus la PV s'approche du cap actualise (N+coupon)*disc.
print("Effet de la barriere (PV en fonction de B) :")
for b in (110.0, 120.0, 130.0, 150.0):
    r = build_autocallable(b).price(model, n_paths=120000, seed=42, steps_per_year=100)
    print(f"  B = {b:6.1f}  ->  PV = {r.price:8.4f}  +/- {r.std_error:.4f}")
print()

# --- Bornes economiques ---
cap = (N + COUPON) * DISC
assert res.price > 0.0, "PV doit etre strictement positive"
assert res.price < cap, f"PV doit rester sous le cap actualise {cap:.4f}"

print("Bornes verifiees : 0 < PV < (N+coupon)*disc")
print(f"RESUME : autocallable B={barrier:.0f} -> PV = {res.price:.2f} "
      f"(cap actualise {cap:.2f}).")
