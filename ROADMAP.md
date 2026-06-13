# ROADMAP — `kontract`

Algèbre des contrats financiers en Rust, exposée à Python via PyO3/maturin.
Fondation théorique : Peyton Jones, Eber, Seward — *"Composing contracts:
an adventure in financial engineering"* (ICFP 2000).

L'implémentation se fait **jalon par jalon**. Chaque jalon est atomique :
il se termine par une suite de tests verts, un commit et un tag git, et une
mise à jour de `PROGRESS.md`. Ne jamais entamer un jalon tant que le précédent
n'est pas marqué `DONE` dans `PROGRESS.md`.

## Convention de modèles

- **Sonnet** : implémentation de code dont l'architecture est déjà fixée.
- **Opus** : jalons de conception (compilateur, planification de calcul,
  early-stopping) où les décisions structurelles sont coûteuses à défaire.
- Chaque jalon Opus est suivi d'une **revue Opus** distincte avant le commit.

## Jalons — Phase fondations (J1–J10)

| # | Titre | Contenu | Modèle | Critère de complétion |
|---|-------|---------|--------|------------------------|
| J1 | AST | Types `Contract`, `Observable`, `Condition` + sérialisation JSON (serde) | Sonnet | Round-trip JSON testé sur 10 contrats |
| J2 | Observables | Évaluation d'observables sur un path donné (`Const`, `Spot`, arithmétique, `Max`) | Sonnet | Tests sur paths synthétiques |
| J3 | Simulateur GBM | Génération de paths vectorisée (`rayon` + `ndarray`), RNG seedable | Sonnet | Moments empiriques vs théoriques (tol. 3 σ) |
| J4 | Compilateur | AST → timeline d'événements + graphe de dépendances (assets, dates, pas fin si barrière) | **Opus** | Timeline correcte sur 5 contrats de référence |
| J5 | Pricer de base | `zero`, `one`, `give`, `and`, `scale`, `when` — MC + discount | Sonnet | Call EU vs Black-Scholes (tol. 1%) |
| J6 | Barrières | `until`, `anytime` avec état d'activation par path | **Opus** | KO call vs formule analytique (tol. 2%) |
| J7 | Greeks | Delta, Vega, Rho par bump-and-reprice (common random numbers) | Sonnet | Greeks call EU vs BS analytique |
| J8 | Bindings PyO3 | `Contract` opaque + surcharge `__add__`, `__neg__`, `>=`, etc. | Sonnet | `import kontract` + pricing depuis Python |
| J9 | Produits | vanilla, asian, knock-out, swaption — suite de validation | Sonnet | Tous prix dans les tolérances |
| J10 | Release | CI GitHub Actions, maturin wheels, PyPI, benchmark vs QuantLib | Sonnet | Wheel installable, bench publié |

## Jalons — Phase modèles avancés (J11–J14)

| # | Titre | Contenu | Modèle | Critère de complétion |
|---|-------|---------|--------|------------------------|
| J11 | Simulator trait | `Simulator` trait pluggable ; refactoriser GBMSimulator ; Pricer agnostique | **Opus** | GBM J5+J6+J7 inchangés, Greeks identiques |
| J12 | Heston + Dupire | `HestonSimulator` (2D vol stochastique) ; `DupireSimulator` (vol locale) | Sonnet | Prix Heston tol. 0.5%, Dupire grid converge |
| J13 | SABR + Sauts | `SABRSimulator` + `MertonJumpSimulator` (compound Poisson) | Sonnet | SABR vs Hagan, Merton vs BS-corrected (6 tests) |
| J14 | Rough Bergomi | `RoughBergomiSimulator` (fBm, Hurst H < 0.5) ; génération Cholesky ou Fourier | **Opus** | Paths bien formés, kurtosis ≠ GBM, tol. 5% |

## Jalons — Phase simulation SOTA (J15–J18)

| # | Titre | Contenu | Modèle | Critère de complétion |
|---|-------|---------|--------|------------------------|
| J15 | Réduction variance | Antithétiques + variables de contrôle (BS ref) ; common RNG | Sonnet | σ² réduit ≥ 50%, prix identique |
| J16 | Quasi-MC (Sobol) | Sobol sequences + Brownian bridge ; convergence O(1/N) vs O(1/√N) | Sonnet | 1/N observable pour N ∈ [100k, 10M] |
| J17 | Américaines (LSM) | Longstaff-Schwartz pour exercice optimal ; backward induction | **Opus** | Put US vs CRR arbres (tol. 0.5%) |
| J18 | Multilevel MC | Hiérarchie pas de temps ; coût O(ε^{-2}) ; tests sur Heston | **Opus** | Économies mesurables à tol. 0.1% |

## Jalons — Phase EDP (J19–J20)

| # | Titre | Contenu | Modèle | Critère de complétion |
|---|-------|---------|--------|------------------------|
| J19 | Crank-Nicolson 1D | EDP Black-Scholes 1D ; PSOR pour américaines ; grille [S_min, S_max] | **Opus** | Call EU vs BS (0.5%), put US vs CRR |
| J20 | ADI 2D | Heston 2D (S, v) ou corrélation 2 actifs ; schéma ADI | **Opus** | Heston call EDP ≈ MC (tol. 1%) |

## Jalons — Phase calibration (J21–J22)

| # | Titre | Contenu | Modèle | Critère de complétion |
|---|-------|---------|--------|------------------------|
| J21 | Lecteur données | yfinance/CSV options ; surface vol implicite (Brent) ; smoothing | Sonnet | S&P 500 surface (skew, term) plausible |
| J22 | Optimiseur calib | Lev-Mar ou CMA-ES ; Heston/Dupire/SABR/Merton ; contraintes param | **Opus** | Round-trip < 1% params, < 0.5% prix |

## Invariants à ne jamais violer

1. L'AST reste **sérialisable et pur** : aucune logique de pricing dedans.
2. Le pricing est **compositionnel** : le prix de `and(a, b)` est dérivé de
   ceux de `a` et `b`, jamais d'un cas spécial.
3. Les Greeks utilisent des **common random numbers** (même seed) pour réduire
   la variance du bump-and-reprice.
4. Toute primitive ajoutée à l'algèbre doit être couverte par un test de path.
5. Le **Simulator trait** (J11) reste le point d'extension principal ; modèles,
   techniques de simulation et EDP s'y branchent, mais ne peuvent pas modifier l'AST.
6. EDP est une **alternative d'exécution**, pas une primitive : le Pricer choisit
   « MC » vs « EDP » au runtime, l'AST ne change pas.
