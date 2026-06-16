# 00 — Tour rapide

## Objectif

Montrer le cycle complet de kontract en une soixantaine de lignes :
construction d'un call européen ATM via le DSL compositionnel, pricing
Monte-Carlo sous GBM, comparaison Black-Scholes, ajout d'une barrière
knock-out, et calcul des Greeks.

## Fonctions kontract illustrées

| Fonction / Méthode | Rôle |
|--------------------|------|
| `k.S("X")` | Observable spot |
| `.clip(0.0)` | Troncature positive (payoff call) |
| `* k.one(k.USD)` | Mise à l'échelle (scale) |
| `@ k.at(T)` | Conditionnement temporel (when) |
| `k.GBM(s0, sigma, r, asset)` | Modèle Black-Scholes-Merton |
| `c.price(model, n_paths, seed)` | Prix Monte-Carlo |
| `c.until(condition)` | Barrière knock-out |
| `c.greeks(model, ...)` | Delta, gamma, vega, rho |

## Lancer le script

```bash
python quickstart.py
```

## Interprétation de la sortie attendue

```
Call ATM 1Y MC   : ~10.44   (BS: ~10.45)   # très faible erreur MC
Call KO H=150    : ~7.91    (rabais: ~2.53) # barrière absorbe ~24% de valeur
Delta            : ~0.637                   # proba risque-neutre ITM
Gamma            : ~0.0123                  # convexité (petit, ATM 1Y)
Vega             : ~37.5                    # sensibilité de 1% de σ ≈ 0.375 USD
```

Le call knock-out vaut moins que le call vanille : si le sous-jacent monte
au-delà de 150, le contrat expire sans valeur. Le delta proche de 0.637
correspond à N(d1) de Black-Scholes.
