"""Twin-Win (gain a la hausse comme a la baisse).

PROFIL DE PAYOFF
----------------
Tant que la barriere basse n'est jamais touchee sur [0,T], l'investisseur
transforme TOUTE variation du sous-jacent (hausse OU baisse) en gain. Le payoff
au-dessus du capital est de type "straddle" autour du spot :
    N + (N/s0) * ( max(S_T - s0, 0) + max(s0 - S_T, 0) )
      = N + (N/s0) * |S_T - s0|

Si la barriere basse est franchie, ce sur-rendement disparait (knock-out) et il
ne reste que le capital N a maturite.

RATIONALE INVESTISSEUR
----------------------
Pari sur la VOLATILITE dans un range : on profite des mouvements dans les deux
sens, a condition que le marche ne s'effondre pas sous la barriere. Le capital
est protege a 100 % a l'echeance.

CONSTRUCTION DSL
----------------
    capital = N one(USD) @ at(T)
    payoff  = ( (S - s0).clip(0) + (s0 - S).clip(0) ) * (N/s0)   # |S - s0|
    twin    = capital + ( (payoff one(USD) @ at(T)).until(S <= low_barrier) )

Le `.until(S <= low_barrier)` desactive la jambe straddle si la barriere basse
est touchee. L'horizon T est porte par les `@ at(T)`.

LIMITE / APPROXIMATION
----------------------
Barriere surveillee en temps DISCRET : la valeur depend de steps_per_year (on
en utilise 100). Avec une barriere continue le knock-out serait plus frequent,
donc la valeur baisserait.
"""

import math

import kontract as k

S0 = 100.0
SIGMA = 0.2
R = 0.05
T = 1.0
N = 100.0
LOW_BARRIER = 80.0
DISC = math.exp(-R * T)
FLOOR = N * DISC

model = k.GBM(s0=S0, sigma=SIGMA, r=R, asset="X")

capital = (k.const_(N) * k.one(k.USD)) @ k.at(T)
straddle_payoff = ((k.S("X") - S0).clip(0.0) + (k.const_(S0) - k.S("X")).clip(0.0)) * (N / S0)
twin_leg = (straddle_payoff * k.one(k.USD) @ k.at(T)).until(k.S("X") <= LOW_BARRIER)
note = capital + twin_leg

res = note.price(model, n_paths=120000, seed=42, steps_per_year=100)

print("=" * 64)
print("TWIN-WIN")
print("=" * 64)
print(f"  Notional N      = {N:.2f}   spot s0 = {S0:.2f}")
print(f"  Barriere basse  = {LOW_BARRIER:.2f}   discount e^-rT = {DISC:.4f}")
print(f"  Plancher N*disc  = {FLOOR:.4f}")
print()
print(f"  PV (MC, 100 pas/an) = {res.price:.4f}  +/- {res.std_error:.4f}")
print(f"  IC95             = [{res.ci95_low:.4f}, {res.ci95_high:.4f}]")
print()

print("Sensibilite a la discretisation de la barriere :")
for spy in (25, 50, 100, 200):
    r = note.price(model, n_paths=120000, seed=42, steps_per_year=spy)
    print(f"  steps_per_year = {spy:4d}  ->  PV = {r.price:8.4f}  +/- {r.std_error:.4f}")
print()

# --- Bornes economiques ---
assert res.price > 0.0, "PV doit etre strictement positive"
assert res.price > FLOOR, "le straddle KO ajoute de la valeur au-dessus du plancher"

print("Bornes verifiees : PV > 0 et PV > plancher N*disc.")
print(f"RESUME : twin-win -> PV = {res.price:.2f} "
      f"(plancher {FLOOR:.2f} + valeur du straddle KO ~{res.price - FLOOR:.2f}).")
