# Exemples Python `kontract`

Catalogue **pédagogique et exhaustif** d'utilisation de la librairie `kontract` depuis Python.
Chaque dossier illustre une famille de fonctionnalités, des **dérivés classiques** (linéaires,
vanilles, européens, américains) aux **exotiques** (path-dependent, multi-actifs, rainbow) et
**produits structurés complexes**. Tous les scripts sont autonomes, impriment leurs résultats et
les comparent à des **références analytiques** (Black-Scholes, parités, Margrabe, Jamshidian,
Garman-Kohlhagen), avec des `assert` qui en font aussi des tests de fumée.

**53 scripts** répartis en **16 dossiers**.

## Pré-requis

```bash
# Construire et installer l'extension Python dans un virtualenv
python -m venv .venv && source .venv/bin/activate
pip install maturin numpy
maturin develop --release --features python
```

## Exécuter

```bash
# Un script précis
python examples/python/00_quickstart/quickstart.py

# Tout le catalogue (échoue au premier script en erreur)
for f in examples/python/*/*.py; do echo "== $f =="; python "$f" || break; done
```

## Sommaire des dossiers

| Dossier | Thème | Fonctions / méthodes `kontract` illustrées |
|---------|-------|---------------------------------------------|
| `00_quickstart` | Tour en 60 secondes | DSL fluide, `.price`, `.until`, `.greeks` |
| `01_dsl_basics` | Primitives & opérateurs | `one/zero/give/S/const_/at`, `@ * + - ~ & |`, `to_json/from_json` |
| `02_linear_derivatives` | Dérivés linéaires | `forward`, prepaid forward, `zero_coupon_bond`, dividende `q`, coupons & annuités |
| `03_vanilla_options` | Vanilles & spreads | `european_call/put`, `straddle`, `bull_call_spread`, collar, butterfly, condor |
| `04_mc_engine` | Moteur Monte-Carlo | diagnostics (`std_error`, `ci95`, `n_paths`), `antithetic`/`control_variate`, `sobol_gbm` |
| `05_greeks` | Sensibilités | `.greeks` (Δ/Γ/ν/ρ) vs BS, scénario sur grille de spot |
| `06_digitals_barriers` | Digitales & barrières | cash/asset-or-nothing, corridor, `up/down_and_out_call`, parité in-out, double KO, rebate |
| `07_path_dependent` | Path-dependent | `average`, `average_over`, `running_max`, `running_min` (asiatiques, lookback) |
| `08_american_bermudan` | Exercice anticipé | `price_american` (LSM), spectre européen → bermudéen → américain |
| `09_models` | Modèles avancés | `heston`, `sabr`, `merton`, `rough_bergomi`, comparatif |
| `10_multi_asset` | Multi-actifs / rainbow | `GbmFactor`, `correlated_gbm`, basket, spread/Margrabe, best-of/worst-of, performance relative |
| `11_rates` | Taux stochastiques | `vasicek`, `hull_white`, `price_under_rates`, `Swaption`, `swaption_mc`, Jamshidian |
| `12_fx` | Change | `garman_kohlhagen_call/put`, `fx_forward`, `quanto_call` |
| `13_calibration` | Calibration | `implied_volatility`, `fit_gbm_volatility` |
| `14_structured_products` | Structurés complexes | autocall, reverse convertible, capital protégé, bonus, discount, twin-win, corridor, shark, booster |
| `15_portfolio_batch` | Portefeuille | pricing groupé, Greeks agrégés, sérialisation JSON d'un livre |

Chaque dossier contient son propre `README.md` détaillant l'objectif, les fonctions illustrées et
l'interprétation des sorties attendues.

## Notes et limites observées

- **`sobol_gbm`** : l'estimateur quasi-MC actuel présente un biais de prix systématique ; le script
  `04_mc_engine/sobol_qmc.py` ne valide que la **réduction de variance**, pas la justesse du prix.
- **`sabr`** : dans l'implémentation actuelle, `ρ`/`ν` n'influencent pas le prix Monte-Carlo
  (documenté dans `09_models/sabr.py`).
- **`fit_gbm_volatility`** : l'optimiseur trust-region démarre à σ₀=0,20 et peut converger
  prématurément ; voir la note dans `13_calibration/fit_gbm_volatility.py`.
- **Produits à barrière continue** (bonus, twin-win, shark) et **corridor mono-fixing** : ce sont
  des approximations dont la valeur dépend de `steps_per_year` ; détaillé dans
  `14_structured_products/README.md`.

> Règle d'horizon : la maturité de simulation est déterminée par le `at(t)` le plus tardif de
> l'arbre du contrat. Un contrat dont la seule temporalité est une barrière de prix (`>=`, `<=`)
> doit comporter un `@ at(T)` quelque part dans l'arbre, sinon il vaut 0.
