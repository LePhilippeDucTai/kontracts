"""Certificat Bonus (Bonus Certificate).

PROFIL DE PAYOFF
----------------
L'investisseur detient le sous-jacent (action prepayee) ET un put "knock-out"
de strike = niveau bonus, qui n'est actif QUE si une barriere basse n'a JAMAIS
ete touchee sur [0,T]. Concretement :
    - si la barriere basse n'est jamais touchee : l'investisseur recoit au
      minimum le niveau bonus (le put KO garantit max(bonus, S_T)),
    - si la barriere basse est touchee : le put disparait (knock-out) et il ne
      reste que l'action ; l'investisseur subit alors la performance brute.

RATIONALE INVESTISSEUR
----------------------
On obtient un "coussin" : un rendement minimal garanti (le bonus) tant que le
marche ne s'effondre pas sous la barriere, tout en conservant l'upside complet
de l'action. C'est un pari sur l'absence de forte baisse.

CONSTRUCTION DSL
----------------
    stock = S one(USD) @ at(T)                                  # action prepayee
    bonus = european_put("X", bonus_level, T, USD)
                .until(S <= low_barrier)                        # put knock-out
    cert  = stock + bonus

Le `.until(S <= low_barrier)` desactive ("knock-out") le put des que la barriere
basse est franchie. L'horizon T provient du `@ at(T)` de la jambe action.

LIMITE / APPROXIMATION
----------------------
La barriere est surveillee en TEMPS DISCRET (steps_per_year pas). Une vraie
barriere continue serait plus souvent franchie ; la valeur converge donc par le
haut quand on augmente steps_per_year. On utilise ici 100 pas/an.
"""

import math

import kontract as k

S0 = 100.0
SIGMA = 0.2
R = 0.05
T = 1.0
BONUS_LEVEL = 110.0   # plancher garanti si barriere intacte
LOW_BARRIER = 80.0    # barriere basse desactivante
DISC = math.exp(-R * T)

model = k.GBM(s0=S0, sigma=SIGMA, r=R, asset="X")

stock = (k.S("X") * k.one(k.USD)) @ k.at(T)
bonus_put = k.european_put("X", BONUS_LEVEL, T, k.USD).until(k.S("X") <= LOW_BARRIER)
cert = stock + bonus_put

res = cert.price(model, n_paths=120000, seed=42, steps_per_year=100)

print("=" * 64)
print("CERTIFICAT BONUS")
print("=" * 64)
print(f"  Spot s0          = {S0:.2f}")
print(f"  Niveau bonus     = {BONUS_LEVEL:.2f}   barriere basse = {LOW_BARRIER:.2f}")
print(f"  discount e^-rT    = {DISC:.4f}")
print()
print(f"  PV (MC, 100 pas/an) = {res.price:.4f}  +/- {res.std_error:.4f}")
print(f"  IC95             = [{res.ci95_low:.4f}, {res.ci95_high:.4f}]")
print()

# Sensibilite a la discretisation (approximation barriere continue)
print("Sensibilite a la discretisation de la barriere :")
for spy in (25, 50, 100, 200):
    r = cert.price(model, n_paths=120000, seed=42, steps_per_year=spy)
    print(f"  steps_per_year = {spy:4d}  ->  PV = {r.price:8.4f}  +/- {r.std_error:.4f}")
print()

# --- Bornes economiques ---
# L'action prepayee vaut s0 (forward actualise) ; le put KO ajoute une valeur > 0.
assert res.price > 0.0, "PV doit etre strictement positive"
assert res.price > S0, "le put bonus KO ajoute de la valeur au-dessus de l'action seule"

print("Bornes verifiees : PV > 0 et PV > s0 (le put KO apporte le coussin bonus).")
print(f"RESUME : certificat bonus -> PV = {res.price:.2f} "
      f"(action {S0:.0f} + coussin bonus ~{res.price - S0:.2f}).")
