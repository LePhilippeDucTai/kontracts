# 08 — Puts américains et bermudéens

## Objectif

Illustrer la valeur de la flexibilité d'exercice anticipé via la méthode
Longstaff-Schwartz (LSM).  Deux scripts comparent :

1. Put européen (1 date d'exercice) vs put américain (exercice mensuel).
2. Le spectre complet : européen → bermudéen → américain.

## Scripts et fonctions illustrées

| Fichier | Produit | API clé | Propriété vérifiée |
|---|---|---|---|
| `american_put_lsm.py` | Put américain vs européen | `payoff.price_american(model, exercise_dates)` | US ≥ EU, prime > 0 |
| `bermudan_put.py` | Spectre EU → Bermudéen → US | `price_american` avec 1, 4 ou 12 dates | EU ≤ Bermudéen ≤ US |

## Comment lancer

```bash
# Activer l'environnement avec kontract installé
source /tmp/j29venv/bin/activate

python examples/python/08_american_bermudan/american_put_lsm.py
python examples/python/08_american_bermudan/bermudan_put.py
```

Les deux scripts utilisent les paramètres `S0=90, K=100, σ=0.30, r=0.08, T=1Y`
(put in-the-money avec taux élevé) pour maximiser la lisibilité de la prime
d'exercice anticipé.  Seed fixée (`seed=42`), `n_paths=120 000`, `n_basis=5`.

## API — exercice anticipé

```python
import kontract as k

S     = k.S("X")
K     = 100.0
model = k.GBM(s0=90, sigma=0.30, r=0.08, asset="X")

# Payoff du put (contrat "exercice")
payoff = (k.const_(K) - S).clip(0.0) * k.one(k.USD)

# Put européen : @ k.at(T) comme d'habitude
eu_put = payoff @ k.at(1.0)
res_eu = eu_put.price(model, n_paths=100_000, seed=42)

# Put américain / bermudéen : price_american avec liste de dates
dates  = [i / 12 for i in range(1, 13)]   # mensuel
res_us = payoff.price_american(model, exercise_dates=dates, n_paths=100_000, seed=42, n_basis=5)
```

La méthode `price_american` accepte n'importe quel sous-ensemble de dates dans
[0, T].  Avec une seule date T, on retrouve le put européen.

## Interprétation des résultats

### Prime d'exercice anticipé

Avec S0=90 < K=100 (in-the-money de 10 %), r=8 % et σ=30 %, la prime
d'exercice anticipé (put américain − put européen) est d'environ **+1.49**
soit **+12 %** du prix européen.

Intuition : détenir le put européen revient à « prêter » K à taux r jusqu'à T.
Si le spot est très bas, il vaut mieux encaisser K − S immédiatement.  Plus
r est élevé, plus ce coût d'attente est lourd → prime plus grande.

### Hiérarchie EU ≤ Bermudéen ≤ US

| Produit | Dates | Prix |
|---|---|---|
| Européen LSM (1 date = T) | 1 | ≈ 12.10 |
| Bermudéen trimestriel | 4 | ≈ 13.41 |
| Américain mensuel | 12 | ≈ 13.59 |

Le passage EU → Bermudéen capture l'essentiel de la prime (~1.31).
Le passage Bermudéen → Américain mensuel n'ajoute qu'environ ~0.17 :
4 dates trimestrielles suffisent à capturer la majorité de la valeur
d'exercice anticipé pour ce profil de paramètres.
