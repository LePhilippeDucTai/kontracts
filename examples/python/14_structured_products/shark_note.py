"""Shark Note (capital protege + up-and-out call + rebate).

PROFIL DE PAYOFF
----------------
Note a capital garanti combinant trois jambes :
    1. capital  : notional N rembourse a maturite T,
    2. up-and-out call : participation a la hausse tant que la barriere haute H
       n'est pas franchie ; si H est touchee, le call est KNOCK-OUT (disparait),
    3. rebate   : un coupon de consolation verse si la barriere H est touchee.

Ainsi l'investisseur participe a la hausse "douce" du sous-jacent, mais si le
marche explose au-dessus de H, il perd l'upside du call et recoit a la place le
rebate. Le capital N est protege dans tous les cas.

RATIONALE INVESTISSEUR
----------------------
Vue haussiere MODEREE : on capture le mouvement tant qu'il reste sous H, avec un
lot de consolation si le marche franchit la barriere. Le call KO coute moins
cher qu'un call vanille, ce qui finance la protection du capital.

CONSTRUCTION DSL
----------------
    capital = N one(USD) @ at(T)
    upside  = const_(N/s0) * up_and_out_call("X", s0, H, T, USD)   # scale du produit
    rebate  = rebate one(USD) .anytime(S >= H)                     # touche -> verse
    note    = capital + upside + rebate

Mise a l'echelle : `Observable * Contract` (ici `const_(N/s0) * produit`) applique
le combinateur `scale` au contrat du catalogue. L'horizon T vient des `@ at(T)`.

LIMITE / APPROXIMATION
----------------------
La barriere up-and-out et le `anytime(S>=H)` du rebate sont surveilles en temps
DISCRET : la valeur depend de steps_per_year (on utilise 100). Barriere continue
=> plus de knock-out et plus de rebate verses.
"""

import math

import kontract as k

S0 = 100.0
SIGMA = 0.2
R = 0.05
T = 1.0
N = 100.0
H = 130.0        # barriere haute (knock-out)
REBATE = 5.0     # coupon de consolation si H touchee
DISC = math.exp(-R * T)
FLOOR = N * DISC

model = k.GBM(s0=S0, sigma=SIGMA, r=R, asset="X")

capital = (k.const_(N) * k.one(k.USD)) @ k.at(T)
upside = k.const_(N / S0) * k.up_and_out_call("X", S0, H, T, k.USD)
rebate = (k.const_(REBATE) * k.one(k.USD)).anytime(k.S("X") >= H)
note = capital + upside + rebate

res = note.price(model, n_paths=120000, seed=42, steps_per_year=100)

print("=" * 64)
print("SHARK NOTE")
print("=" * 64)
print(f"  Notional N      = {N:.2f}   spot s0 = {S0:.2f}")
print(f"  Barriere H      = {H:.2f}   rebate = {REBATE:.2f}")
print(f"  discount e^-rT   = {DISC:.4f}   plancher N*disc = {FLOOR:.4f}")
print()
print(f"  PV (MC, 100 pas/an) = {res.price:.4f}  +/- {res.std_error:.4f}")
print(f"  IC95             = [{res.ci95_low:.4f}, {res.ci95_high:.4f}]")
print()

# Decomposition (a titre pedagogique).
# NB: la jambe rebate ne porte AUCUN at(T) ; isolee elle violerait la regle
# d'horizon et coterait 0. Pour mesurer sa contribution reelle on lui adjoint un
# sibling zero @ at(T) qui fournit l'horizon de monitoring (cf. CRITICAL HORIZON RULE).
horizon_anchor = (k.const_(0.0) * k.one(k.USD)) @ k.at(T)
pv_uoc = upside.price(model, n_paths=120000, seed=42, steps_per_year=100).price
pv_reb = (rebate + horizon_anchor).price(model, n_paths=120000, seed=42, steps_per_year=100).price
print("Decomposition des jambes :")
print(f"  capital (N*disc)          ~ {FLOOR:8.4f}")
print(f"  up-and-out call (scale)    = {pv_uoc:8.4f}")
print(f"  rebate (anytime S>=H, +ancre at(T)) = {pv_reb:8.4f}")
print()

print("Sensibilite a la discretisation de la barriere :")
for spy in (50, 100, 200):
    r = note.price(model, n_paths=120000, seed=42, steps_per_year=spy)
    print(f"  steps_per_year = {spy:4d}  ->  PV = {r.price:8.4f}  +/- {r.std_error:.4f}")
print()

# --- Bornes economiques ---
EPS = 3.0 * res.std_error + 1e-6
assert res.price > 0.0, "PV doit etre strictement positive"
assert res.price >= FLOOR - EPS, "PV >= plancher N*disc (capital protege)"

print("Bornes verifiees : PV >= plancher N*disc (capital protege).")
print(f"RESUME : shark note -> PV = {res.price:.2f} "
      f"(plancher {FLOOR:.2f} + call KO {pv_uoc:.2f} + rebate ~{pv_reb:.2f}).")
