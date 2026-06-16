# PLAN_EXEMPLES — Catalogue exhaustif d'exemples Python `kontract`

Ce document décrit le catalogue d'exemples d'utilisation Python de `kontract` : son périmètre,
le **modèle** et l'**effort** nécessaires par partie, et un indicateur de **parallélisabilité**
pour l'implémentation. Il sert de référence à la création des exemples sous `examples/python/`.

## Contexte

`kontract` expose une algèbre de contrats financiers à Python (PyO3). Ce catalogue est
**pédagogique et exhaustif** : il couvre **toute** la surface d'API exposée, des **dérivés
classiques** (linéaires, vanilles, européens, américains) aux **exotiques** (path-dependent,
multi-actifs, rainbow) et **produits structurés complexes** (autocall, reverse convertible,
notes à capital protégé, bonus/discount/twin-win/shark/booster). Chaque dossier est documenté
par un `README.md` en français.

Conventions :
- Emplacement **`examples/python/`** (hors `python-source` du wheel → pas de pollution du package).
- **Smoke test CI** : job dédié exécutant chaque script (échec si un script casse).
- Style **pédagogique** : commentaires du « pourquoi », **sorties imprimées** (prix, IC 95 %,
  Greeks, bornes), **comparaisons aux références analytiques** (Black-Scholes, parités, Margrabe,
  Jamshidian, Garman-Kohlhagen) et `assert` de validation.

## Couverture d'API (100 % de la surface exposée)

DSL & opérateurs (`S/spot/const_/one/zero/give/at`, `@→when`, `*→scale`, `+→and`, `-unaire→give`,
`~→not`, `&`/`|`, comparaisons→`Condition`) ; `to_json`/`from_json` ; pricing (`price`,
`price_american`, `price_under_rates`, `greeks`) ; modèles (`GBM`+dividende `q`, `heston`, `sabr`,
`merton`, `rough_bergomi`, `sobol_gbm`) ; observables temporels (`average`, `average_over`,
`running_max`, `running_min`) ; multi-actifs (`GbmFactor`, `correlated_gbm`) ; taux (`vasicek`,
`hull_white`, `Swaption.level`, `swaption_mc`, `vasicek_swaption_analytic`,
`RateModel.zero_bond/discount_bond0`) ; FX (`garman_kohlhagen_call/put`, `fx_forward`,
`quanto_call`) ; catalogue produits (`european_call/put`, `forward`, `straddle`,
`bull_call_spread`, `cash_or_nothing_call`, `up_and_out_call`, `down_and_out_call`,
`zero_coupon_bond`) ; calibration (`implied_volatility`, `fit_gbm_volatility`) ; résultats
(`PriceResult`, `Greeks`) ; réduction de variance (`antithetic`, `control_variate`).

Les **produits structurés J28** (Rust) ne sont pas bindés → **reconstruits depuis les primitives
DSL** en Python (démonstration de compositionnalité), avec d'autres structurés exotiques.

## Arborescence (`examples/python/`)

```
examples/python/
├── README.md                          # index + pré-requis + commande "tout exécuter"
├── 00_quickstart/quickstart.py        # tour en 60 s : call → prix/IC, barrière, Greeks
├── 01_dsl_basics/                     # building_blocks, operators, serialization
├── 02_linear_derivatives/            # forward_prepaid, futures_carry_dividends, bonds_annuities
├── 03_vanilla_options/               # european_call_put, straddle_strangle, spreads_collar, butterfly_condor
├── 04_mc_engine/                     # mc_diagnostics, variance_reduction, sobol_qmc
├── 05_greeks/                        # greeks, greeks_scenario
├── 06_digitals_barriers/             # digitals, knock_out, knock_in_parity, double_barrier, barrier_rebate
├── 07_path_dependent/                # asian_fixed_strike, asian_floating_strike, asian_windowed, lookback_fixed, lookback_floating
├── 08_american_bermudan/             # american_put_lsm, bermudan_put
├── 09_models/                        # heston, sabr, merton, rough_bergomi, model_comparison
├── 10_multi_asset/                   # basket, spread_margrabe, best_of_worst_of, outperformance
├── 11_rates/                         # zero_coupon_stochastic, coupon_bond, swaptions
├── 12_fx/fx_options.py               # GK call/put + parité, fx_forward (IRP), quanto (monotone ρ)
├── 13_calibration/                   # implied_vol, fit_gbm_volatility
├── 14_structured_products/           # autocallable, reverse_convertible, capital_protected_note,
│                                     #   bonus_certificate, discount_certificate, twin_win,
│                                     #   corridor_note, shark_note, booster_note
└── 15_portfolio_batch/portfolio.py   # livre de contrats : pricing groupé, Greeks agrégés, JSON
```

Total ≈ **16 README.md** (1 index + 15 dossiers) et **~46 scripts** `.py`.

## Tableau d'implémentation — modèle, effort, parallélisation

Échelle d'effort : **S** (court, API connue, params validés) · **M** (composition + référence
analytique) · **L** (construction exotique + bornes économiques + limites documentées).
Modèle : **Sonnet** pour le mécanique ; **Opus** pour les structurés exotiques (construction DSL
et justification économique délicates).

| Lot | Dossiers | Modèle | Effort | Parallélisable | Dépendances |
|-----|----------|--------|--------|----------------|-------------|
| **A. Fondations DSL** | 00_quickstart, 01_dsl_basics | Sonnet | S–M | ✅ | aucune |
| **B. Classiques linéaires & vanilles** | 02_linear_derivatives, 03_vanilla_options | Sonnet | M | ✅ | aucune |
| **C. Moteur MC & Greeks** | 04_mc_engine, 05_greeks | Sonnet | M | ✅ | aucune |
| **D. Digitals & barrières** | 06_digitals_barriers | Sonnet | M | ✅ | aucune |
| **E. Path-dependent** | 07_path_dependent | Sonnet | M | ✅ | aucune |
| **F. Américaines/Bermudéennes** | 08_american_bermudan | Sonnet | M | ✅ | aucune |
| **G. Modèles avancés** | 09_models | Sonnet | M | ✅ | aucune |
| **H. Multi-actifs / rainbow** | 10_multi_asset | Sonnet | M–L | ✅ | aucune |
| **I. Taux & swaptions** | 11_rates | Sonnet | M | ✅ | aucune |
| **J. FX** | 12_fx | Sonnet | S–M | ✅ | aucune |
| **K. Calibration** | 13_calibration | Sonnet | S–M | ✅ | aucune |
| **L. Structurés exotiques** | 14_structured_products | **Opus** | **L** | ✅ entre eux | relire bornes |
| **M. Portefeuille / batch** | 15_portfolio_batch | Sonnet | M | ✅ | réutilise B/D |
| **N. Index + smoke-test CI** | README racine examples, `.github/workflows/ci.yml` | Sonnet | S | ⛔ après A–M | liste finale |
| **O. Vérification** | exécution de tous les scripts + fmt/clippy/pytest | Sonnet | M | ⛔ séquentiel | tout |

**Stratégie de parallélisation** : les lots **A–M sont mutuellement indépendants** (chaque script
est autonome, ré-implémente sa propre référence `bs_call`, importe seulement `kontract`). Ils
peuvent être confiés à plusieurs agents en parallèle. Les lots **N** (index + CI) et **O**
(vérification globale) sont **séquentiels** et viennent après.

## Validité DSL des exotiques (constructions vérifiées)

- **Knock-in par parité** : `in = vanille + (−out)` (pas de `__sub__` sur `Contract`).
- **Double KO** : `call.until((S>=H) | (S<=L))`.
- **Rebate au touch** : `(const_(R) * one(USD)).anytime(S>=H)`, ajouté au KO.
- **Corridor digital** : `when(at(T), when((S>=L) & (S<=H), const_(payout)*one(USD)))`.
- **Asset-or-nothing** : `when(at(T), when(S>=K, S("X")*one(USD)))`.
- **Asian/lookback** : `average`, `average_over`, `running_max/min` (grille fine `steps_per_year≥50`).
- **Rainbow best-of/worst-of** : `S("S1").max(S("S2"))` / `.min(...)` sous `correlated_gbm`.
- **Outperformance** : `(S("S1")/s1_0) - (S("S2")/s2_0)` puis `.clip(0.0)`.
- **Discount certificate** : `min(S_T,K) = (S("X")*one@T) + (−european_call)`.
- **Autocallable** : `payoff.anytime(S>=B) + redemption.until(S>=B)`.
- **Reverse convertible** : `(bond+coupon) + (−put)` (`give`).
- **Shark/booster** : `zcb + call.until(S>=H) + rebate.anytime(S>=H)` ; `zcb + 2*bull_spread`.

**Limites documentées dans les README** : le **range accrual** multi-fixing n'est pas exprimable
(pas d'observable « indicatrice moyennée ») → remplacé par un **corridor note mono-fixing** ;
**bonus/twin-win** sont des **approximations** de barrière à fenêtre continue (valeur dépendante de
`steps_per_year`) ; Asian/lookback dépendent du pas de grille.

## Pièges d'API (vérifiés dans `src/bindings.rs`)

- `Contract` n'a **pas** `__sub__` → jambe vendue = `c1 + (-c2)`.
- Pas de `__rtruediv__` → éviter `scalaire / observable` (`100.0 - S("X")` OK via `__rsub__`).
- Précédence Python : `@` au même niveau que `*`/`/` → **parenthéser** le scale :
  `((S("X") - K).clip(0.0) * one(USD)) @ at(T)`.
- `price_under_rates` ignore le taux du `GBM` (actualisation = `RateModel`).
- `correlated_gbm` lève `ValueError` si la matrice n'est pas N×N.

## Intégration CI (smoke test)

Job **`examples`** dans `.github/workflows/ci.yml` (ubuntu-latest, Python 3.11) : build du wheel
(`maturin build --release --features python`), installation, puis exécution de chaque script
(`for f in examples/python/*/*.py; do python "$f" || exit 1; done`).

## Vérification (end-to-end)

1. `maturin develop --release` dans un venv.
2. Exécuter **chaque** script : code 0, sorties sensées, `assert` (prix ≈ référence, monotonies,
   parités, bornes) verts.
3. Non-régression : `cargo fmt --all -- --check`, `cargo clippy --features python --all-targets
   -- -D warnings`, `python -m pytest python/tests -q` restent verts.
