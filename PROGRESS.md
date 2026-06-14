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
| J4 | Compilateur | DONE | j4-compiler | [Opus + revue] `compile(&Contract)` → `Plan{assets, fixed_dates, horizon, needs_fine_grid}` ; barrière = condition dépendante du prix sous when/until/anytime ; `Plan::time_grid` (européen = dates seules, barrière = grille dense) ; 9 tests dont 5 contrats de référence |
| J5 | Pricer de base | DONE | j5-pricer | `price_gbm` compositionnel : réduction en flux `(montant, date)` par trajectoire, `scale` échantillonné à la date du flux, actualisation déterministe `e^{-rt}`, moyenne rayon ; `McConfig`/`PriceResult` ; or/until/anytime → `Unsupported` (J6) ; call EU vs BS < 1 % (9 tests) |
| J5b | MC Diagnostics | DONE | j5b-diagnostics | `PriceResult` enrichi : `sample_std`, `std_error = σ/√n`, IC 95 % (`ci95_low/high`), `n_paths` ; `paths_for_tolerance(tol)` = `(1.96σ/tol)²` ; IC contient le prix BS, scaling quadratique vérifié (5 tests) |
| J6 | Barrières | DONE | j6-barriers | [Opus + revue] `Condition::eval(path,t)` (couche numérique) ; `until` = knock-out (flux strictement antérieurs à 1ʳᵉ activation) ; `anytime` = first-touch (optimal → J17) ; `when` étendu aux conditions prix ; activation par path via `first_activation` ; `or` → Unsupported (J17) ; KO call vs Reiner-Rubinstein+BGK < 2 % (6 tests) |
| J7 | Greeks | DONE | j7-greeks | `greeks_gbm` bump-and-reprice avec common random numbers (graine constante → variance effondrée) ; Δ/Γ (diff. spot), Vega (diff. vol), Rho (diff. taux = drift+discount) ; `Greeks`/`BumpSizes` ; vs BS : Δ<1 %, Vega/Rho<2 %, Γ<5 % (5 tests) |
| J7b | Surfaces Greeks | DONE | j7b-surface | `greek_surface` : grille `(spot×vol)` de prix/δ/γ/ν évaluée en parallèle (rayon) ; `GreekSurface` (Array2) + export `to_csv` et `to_pgm` (heatmap grayscale sans dépendance) ; surfaces δ/ν vs BS (<2 %/<3 %), monotonie δ (5 tests) |
| J8a | API Python ergonomique | DONE | j8a-dsl | DSL fluide Rust : arithmétique observable⊕scalaire (2 sens), `.clip(floor)`, `observable * contract`/`f64 * contract` = scale, méthodes `.when/.until/.anytime/.and/.or/.give`, alias `s()`, constantes devises USD/EUR/GBP/JPY ; `@` Python → `when` mappé en J8 ; call fluide ≡ verbeux, 10 contrats en une ligne (5 tests) |
| J8 | Bindings PyO3 | DONE | j8-pyo3 | Bindings PyO3 0.21 : classes `Observable`/`Condition`/`Contract`/`GBM`/`PriceResult`/`Greeks` ; opérateurs Python (`- * / @ + ~`, comparaisons → Condition) ; `Contract.price()/greeks()` ; constructeurs `S/spot/one/zero/at/const_` + devises ; build maturin (venv) OK ; 6 tests pytest (import, fluide `@`, call vs BS <1 %, greeks, KO, portefeuille) |
| J9c | Batch pricing | DONE | j9c-batch | `price_on_paths` (éval sur trajectoires pré-simulées) + `price_batch_gbm` : grille unifiée (union des dates, fine si barrière), **simulation unique partagée**, éval parallèle rayon par contrat ; batch ≡ pricing individuel ; **100 contrats en 0,14 s en release** (< 500 ms) ; 4 tests |
| J9 | Produits validation | DONE | j9-products | Module `products` (catalogue d'expressions DSL, moteur agnostique) : ZC, call/put EU, forward, straddle, bull spread, digital cash-or-nothing, up/down-and-out ; tous validés vs formules fermées (9 tests). **Asian reporté** (pas d'observable d'agrégation temporelle), **swaption reportée** (taux stochastiques → J24) |
| J10 | Release | DONE | j10-release | CI GitHub Actions (fmt + clippy 2 modes + `cargo test --release` + build wheel & pytest) ; workflow release multi-OS (maturin-action → PyPI sur tag `v*`) ; **wheel release construite et installée dans un venv neuf** (import + pricing OK, call ATM 10,44) ; README mis à jour. Phase 1 (MVP Trader) terminée ✅ |

## Phase 2 : Modèles avancés (semaines 4–8)

| Jalon | Titre | Statut | Tag git | Résumé décisions |
|-------|-------|--------|---------|------------------|
| J11 | Simulator trait | DONE | j11-simulator | `Simulator` trait (`simulate`, `simulate_paths`, `asset_name`) ; implémentation `Gbm` sans modification logique ; `price_gbm`/`price_batch_gbm` acceptent `&dyn Simulator` ; pricer agnostique au modèle ; 91 tests verts, régression J1–J10 vérifiée ; déverrouille J12–J14 (Heston, Dupire, SABR, Rough Bergomi) |
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
