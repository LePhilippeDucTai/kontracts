# PROGRESS — `kontract`

Mis à jour automatiquement par Claude Code à la fin de chaque jalon.
Statuts possibles : `TODO`, `IN_PROGRESS`, `DONE`.

**Trader MVP target** : Phase 1 complète (J1–J10) en 2–3 semaines.
**Ordre d'exécution strict** : J1 → J2 → J3 → J4 → J5 → J5b → J6 → J7 → J7b → J8a → J8 → J9c → J9 → J10 → J11 → ...

## Phase 1 : MVP Trader (semaines 1–3)

| Jalon | Titre | Statut | Tag git | Résumé décisions |
|-------|-------|--------|---------|------------------|
| J1 | AST | DONE | j1-ast | Enums `Contract`/`Observable`/`Condition` purs + serde JSON ; constructeurs DSL + ops surchargés (`+ - * / neg`, `max/min`, `ge/gt/le/lt`, `!`) ; round-trip vérifié sur 10 contrats |
| J2 | Observables | DONE | j2-observable | `Path` (grille + spots par actif) + `Observable::eval(path, t)` ; logique numérique isolée hors AST ; erreurs UnknownAsset/StepOutOfRange/InconsistentPath ; 8 tests (payoffs call/put, spread, nested) |
| J3 | GBM Simulateur | DONE | j3-gbm | `Gbm::simulate` → `Array2[n_paths,n_steps]` schéma log-normal exact ; rayon par trajectoire ; RNG ChaCha8 seedé par (seed,index) → reproductible indép. de l'ordre ; `simulate_paths` → `Vec<Path>` ; moments empiriques vs théorie OK (8 tests) |
| J4 | Compilateur | TODO | — | |
| J5 | Pricer de base | TODO | — | |
| J5b | MC Diagnostics | TODO | — | |
| J6 | Barrières | TODO | — | |
| J7 | Greeks | TODO | — | |
| J7b | Surfaces Greeks | TODO | — | |
| J8a | API Python ergonomique | TODO | — | |
| J8 | Bindings PyO3 | TODO | — | |
| J9c | Batch pricing | TODO | — | |
| J9 | Produits validation | TODO | — | |
| J10 | Release | TODO | — | |

## Phase 2 : Modèles avancés (semaines 4–8)

| Jalon | Titre | Statut | Tag git | Résumé décisions |
|-------|-------|--------|---------|------------------|
| J11 | Simulator trait | TODO | — | |
| J12 | Heston + Dupire | TODO | — | |
| J13 | SABR + Merton | TODO | — | |
| J14 | Rough Bergomi | TODO | — | |
| J15 | Réduction variance | TODO | — | |
| J16 | Quasi-MC | TODO | — | |
| J17 | Américaines (LSM) | TODO | — | |
| J18 | Multilevel MC | TODO | — | |
| J21 | Lecteur données | TODO | — | |
| J21-fast | Calibration rapide (< 1 sec) | TODO | — | |

## Phase 3 : Risk Manager (optionnel)

| Jalon | Titre | Statut | Tag git | Résumé décisions |
|-------|-------|--------|---------|------------------|
| J19 | Crank-Nicolson 1D | TODO | — | |
| J20 | ADI 2D | TODO | — | |
| J22 | Optimiseur complet (CMA-ES) | TODO | — | |
| J23 | Backtesting | TODO | — | |

## Phase 4 : Extension future

| Jalon | Titre | Statut | Tag git | Résumé décisions |
|-------|-------|--------|---------|------------------|
| J24 | Taux stochastiques (Vasicek/HW) | TODO | — | |
| J25 | FX simple | TODO | — | |

## Journal d'implémentation

<!-- Chaque jalon complété ajoute une entrée : date | Jx | titre | décisions clés prises -->
