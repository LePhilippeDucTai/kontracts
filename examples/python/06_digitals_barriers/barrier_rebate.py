"""
Barrière knock-out avec rebate au toucher.

Un rebate est un paiement immédiat versé au porteur dès que la barrière
est franchie (consolation prize). Il s'exprime en DSL par :

    rebate = (k.const_(5.0) * k.one(k.USD)).anytime(k.S("X") >= H)

`.anytime(cond)` donne le droit d'exercer immédiatement dès que la
condition est vérifiée à n'importe quel pas de temps.

Combinaison avec le knock-out :
    c_ko_rebate = c_ko + rebate

Le `k.up_and_out_call` fournit l'horizon `at(T)` dont le pricer a besoin
pour traverser le temps. Le rebate, ajouté par `+`, hérite de cet horizon.

Pour évaluer la valeur du rebate seul (sans le KO), on ajoute un porteur
d'horizon neutre : `k.zero() @ k.at(T)`, qui ne paie rien mais fixe T.

Propriété vérifiée :
    Prix(KO + rebate) > Prix(KO)   — le rebate a une valeur positive.
"""

import math
import kontract as k

# ── Paramètres ─────────────────────────────────────────────────────────────
S0, K, r, sigma, T = 100.0, 100.0, 0.05, 0.20, 1.0
BARRIER = 130.0
REBATE  = 5.0
N_PATHS = 100_000
SEED    = 42
STEPS   = 100   # monitoring continu approché


# ── Contrats ───────────────────────────────────────────────────────────────
m = k.GBM(s0=S0, sigma=sigma, r=r, asset="X")

# Call up-and-out (fournit l'horizon at(T) interne)
c_ko = k.up_and_out_call("X", K, BARRIER, T, k.USD)

# Rebate : 5 USD versés dès que S touche la barrière haute
rebate = (k.const_(REBATE) * k.one(k.USD)).anytime(k.S("X") >= BARRIER)

# KO + rebate : combinaison naturelle en DSL
c_ko_rebate = c_ko + rebate

# Rebate seul : on ajoute un porteur d'horizon neutre pour fixer T
#   (k.zero() @ k.at(T) paie 0 mais informe le pricer que l'horizon est T)
c_rebate_seul = rebate + (k.zero() @ k.at(T))

# ── Pricing ────────────────────────────────────────────────────────────────
kwargs = dict(n_paths=N_PATHS, seed=SEED, steps_per_year=STEPS)

r_ko          = c_ko.price(         m, **kwargs)
r_rebate_seul = c_rebate_seul.price(m, **kwargs)
r_ko_rebate   = c_ko_rebate.price(  m, **kwargs)

# ── Affichage ──────────────────────────────────────────────────────────────
print(f"KO avec rebate au toucher : S={S0}, K={K}, H={BARRIER}, rebate={REBATE}")
print(f"  r={r}, σ={sigma}, T={T}")
print(f"  steps_per_year={STEPS}, n_paths={N_PATHS}, seed={SEED}\n")

print(f"  Prix KO seul                         : {r_ko.price:.4f}")
print(f"  Prix rebate seul (via horizon neutre) : {r_rebate_seul.price:.4f}")
print(f"  Prix KO + rebate                     : {r_ko_rebate.price:.4f}")

valeur_rebate_incrementale = r_ko_rebate.price - r_ko.price
print(f"\n  Valeur incrémentale du rebate         : {valeur_rebate_incrementale:.4f}")
print(f"  Cohérence rebate seul / incrémental   : "
      f"{abs(r_rebate_seul.price - valeur_rebate_incrementale):.5f} (doit être ≈ 0)")

# Probabilité de toucher la barrière ≈ rebate_MC / (rebate * e^{-r*temps_moy})
# Estimation grossière : si rebate est versé au temps de touch τ (≈ T/2 en moy)
prob_touch_approx = r_rebate_seul.price / REBATE
print(f"\n  Probabilité approchée de toucher H    : {prob_touch_approx:.4%}")
print(f"  (si versé en moyenne à t=T/2 : P ≈ rebate_MC / rebate = {prob_touch_approx:.4%})")

# ── Vérifications ──────────────────────────────────────────────────────────
# KO + rebate > KO
assert r_ko_rebate.price > r_ko.price, (
    f"KO+rebate ({r_ko_rebate.price:.4f}) doit être > KO ({r_ko.price:.4f})"
)

# Rebate positif
assert r_rebate_seul.price > 0.0, "Valeur du rebate doit être positive"

# Cohérence entre mesure directe du rebate et différence KO/KO+rebate
assert abs(r_rebate_seul.price - valeur_rebate_incrementale) < 0.05, (
    f"Incohérence rebate seul ({r_rebate_seul.price:.4f}) vs incrémental "
    f"({valeur_rebate_incrementale:.4f})"
)

# KO+rebate < vanille (malgré le rebate, on perd le payoff du call si la barrière est touchée)
c_vanilla = k.european_call("X", K, T, k.USD)
r_vanilla = c_vanilla.price(m, **kwargs)
assert r_ko_rebate.price < r_vanilla.price, (
    f"KO+rebate ({r_ko_rebate.price:.4f}) doit rester < vanille ({r_vanilla.price:.4f})"
)

print(f"\n  Vanille (référence) : {r_vanilla.price:.4f}")
print(f"  KO+rebate < vanille : {r_ko_rebate.price:.4f} < {r_vanilla.price:.4f}")
print("\n✓ Toutes les assertions passent.")
