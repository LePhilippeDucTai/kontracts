# 06_digitals_barriers — Options digitales et à barrière

## Objectif

Illustrer les options binaires (digitales) et à barrière (knock-out, knock-in)
construites avec le DSL `kontract`. Ces produits n'existent pas comme primitives
dans le moteur : ils sont composés à partir des combinateurs `when`, `until`,
`anytime` et des observables `k.S("X")`.

## Scripts

| Fichier | Ce qu'il montre |
|---|---|
| `digitals.py` | Cash-or-nothing, asset-or-nothing (DSL pur), corridor digital |
| `knock_out.py` | Up-and-out et down-and-out < vanille ; barrière lointaine ≈ vanille |
| `knock_in_parity.py` | Parité KI + KO = vanille |
| `double_barrier.py` | Double KO avec `.until((S≥H_haut)|(S≤H_bas))` |
| `barrier_rebate.py` | KO + rebate au toucher via `.anytime()` |

## Comment lancer

```bash
python digitals.py
python knock_out.py
python knock_in_parity.py
python double_barrier.py
python barrier_rebate.py
```

## Interprétation des sorties

### digitals.py

- **Cash-or-nothing** : utilise le produit catalogue `k.cash_or_nothing_call`.
  Prix analytique = payout × e^{-rT} × N(d₂).
- **Asset-or-nothing** : construit en DSL avec un double `@`.
  `(k.S("X") * k.one(k.USD)) @ (k.S("X") >= K) @ k.at(T)` :
  - Le premier `@` conditionne le payoff au niveau du spot final.
  - Le second `@` fournit l'**horizon temporel** T — sans lui le pricer
    ne traverserait pas le temps et retournerait 0.
  Prix analytique = S₀ × N(d₁).
- **Corridor** : condition composée `(S≥95) & (S≤105)`.
  Prix analytique = 10 × e^{-rT} × (N(d₂(95)) − N(d₂(105))).

### knock_out.py

Le prix KO est toujours inférieur à la vanille car la barrière peut annuler
le payoff. Une barrière très éloignée (H=300) ne se déclenche presque jamais
et donne un prix quasi-identique à la vanille (écart < 0.2 %).

### knock_in_parity.py

La parité KI + KO = vanille est une identité exacte : chaque chemin de prix
finit soit KI (barrière touchée) soit KO (non touchée), et jamais les deux.
L'assertion vérifie la parité à 3 × (std_error_vanilla + std_error_ko).

### double_barrier.py

`.until((k.S("X") >= 130) | (k.S("X") <= 80))` annule le contrat dès que
l'une des deux barrières est franchie. Le prix est légèrement inférieur au
single up-and-out car la barrière basse constitue une contrainte supplémentaire
(ici peu mordante, le spot démarrant à 100 loin de la barrière basse à 80).

### barrier_rebate.py

`.anytime(cond)` déclenche un exercice immédiat dès que la condition est vraie
sur n'importe quel pas de temps. Pour pricer le rebate seul, on lui adjoint un
porteur d'horizon neutre `k.zero() @ k.at(T)` afin que le pricer parcourt
le temps jusqu'à T. La valeur du rebate seul coïncide avec la valeur
incrémentale (KO+rebate) − KO, confirmant la décomposition additive.
