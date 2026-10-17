# Changelog

Format : [Keep a Changelog](https://keepachangelog.com/), versionnage sémantique.

## [0.2.3] — 2026-06-16

### Changed
- Wheels précompilées limitées à **Linux ARM64** et **Windows x64** (retrait de
  Linux x64 et des deux cibles macOS de la matrice) ; `README` documente
  l'installation directe depuis les assets de la GitHub Release.

## [0.2.2] — 2026-06-16

### Changed
- Workflow `Release` : runners macOS migrés vers `macos-15` / `macos-15-intel`.

## [0.2.1] — 2026-06-16

### Added
- **Catalogue d'exemples Python** (`examples/python/`, cf. `PLAN_EXEMPLES.md`) :
  53 scripts + 16 READMEs couvrant 100 % de la surface d'API — DSL, dérivés
  linéaires, vanilles, moteur MC (diagnostics, réduction de variance, Sobol),
  Greeks, digitales/barrières, path-dependent (Asian/lookback), américaines,
  modèles avancés, multi-actifs, taux, FX, calibration, produits structurés
  et portefeuille batch — chacun validé par `assert` contre des références
  analytiques ; job CI `examples` (smoke test exécutant chaque script).
- **Workflow `Release`** : wheels précompilées attachées aux GitHub Releases
  (installation sans toolchain Rust), en remplacement de la publication PyPI.

### Fixed
- Test performance batch (100 contrats < 500 ms) : assertion active
  uniquement en `--release` (le temps en debug dépend de la machine).

## [0.2.0] — 2026-06-15

Durcissement production (post-J25).

### Added
- **Bindings Python étendus** : `Contract.price()` accepte tout modèle
  (`GBM` + `Model` via `heston`/`sabr`/`merton`/`rough_bergomi`/`sobol_gbm`) ;
  dividende `q` sur `GBM` ; réduction de variance (`antithetic`/`control_variate`) ;
  `price_american` (LSM) ; `price_under_rates` (taux stochastiques) ; catalogue de
  produits, taux (`vasicek`/`hull_white`/`Swaption`/`swaption_mc`), FX
  (`garman_kohlhagen_*`, `fx_forward`, `quanto_call`), calibration
  (`implied_volatility`, `fit_gbm_volatility`). Stubs `.pyi` + `py.typed`.
- **Tests de propriété** (`proptest`) et **benchmarks** (`criterion`).
- **CI** : matrice 3 OS × Python 3.9–3.12 + couverture (`cargo-llvm-cov`).
- Wheels **abi3** (une seule wheel CPython ≥ 3.9) ; métadonnées de paquet
  (repository, keywords, categories) ; `LICENSE`, `CHANGELOG`, exemple Rust.

### Fixed
- **PDE J19 (Crank-Nicolson 1D)** : coefficients corrigés (diffusion 2× trop
  grande, signe du drift implicite) + bord par linéarité ⇒ call ATM vs BS
  2,4 % → 0,001 %.
- **PDE J20 (ADI 2D Heston)** : réécrit en schéma de Douglas avec terme croisé
  `ρσvS·∂²V/∂S∂v` explicite ⇒ vs MC Heston < 0,5 % (était « <30 % »).
- **`market_data::norm_cdf`** : facteur `1/√2` manquant (vol implicite biaisée).
- **Sérialisation AST** : `serde_json` parsait les f64 à 1 ULP près →
  feature `float_roundtrip` (round-trip exact).

## [0.1.0]

MVP Trader (J1–J10) + modèles avancés, calibration, EDP, taux et FX (J11–J25).
Voir `PROGRESS.md` pour le détail jalon par jalon.
