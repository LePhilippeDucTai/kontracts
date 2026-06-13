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
- **Opus** : jalons de conception où les décisions structurelles sont coûteuses à défaire.
- Chaque jalon Opus est suivi d'une **revue Opus** distincte avant le commit.

## Stratégie : Trader Quant MVP en 2–3 semaines

**Phase 1 (J1–J9c)** : Trader quant peut pricer, Greeks, scenarios sur GBM en 2–3 semaines.
**Phase 2 (J11–J18)** : Modèles avancés + calibration (4–6 semaines).
**Phase 3 (J19–J23)** : Risk manager features (EDP, backtesting) — optionnel.

## Phase 1 : MVP Trader (semaines 1–3)

| # | Titre | Contenu | Modèle | Critère | Trader use |
|---|-------|---------|--------|---------|------------|
| J1 | AST | `Contract`, `Observable`, `Condition` + JSON | Sonnet | Round-trip sur 10 contrats | Sérialiser |
| J2 | Observables | Éval sur path (`Spot`, arithmétique, `Max`, etc.) | Sonnet | Tests paths synthétiques | Payoffs |
| J3 | GBM Simulateur | Paths vectorisés (`rayon`), RNG seedable | Sonnet | Moments empiriques vs théorie | Scénarios |
| J4 | Compilateur | AST → timeline + graphe de dépendances | **Opus** | Timeline 5 contrats | Validation |
| J5 | Pricer base | `zero`, `one`, `give`, `and`, `scale`, `when` + discount | Sonnet | Call EU vs BS (1%) | **Pricer book** |
| J5b | MC Diagnostics | Erreur standard, CI 95%, `n_paths` nécessaire | Sonnet | Trader sait son erreur MC | **Incertitude** |
| J6 | Barrières | `until`, `anytime` (activation par path) | **Opus** | KO call vs analytique (2%) | Knock-outs |
| J7 | Greeks | Δ, Γ, Ν par bump-and-reprice (CRN) | Sonnet | Greeks EU call vs BS | **Hedging** |
| J7b | Surfaces Greeks | Grilles δ(S, σ), γ(S, σ), ν(S, σ) + heatmaps | Sonnet | Surfaces vs BS analytique | **Scenarios** |
| J8a | API Python | Surcharges ergonomiques : `(S-K).clip(0) * one(USD) @ at(T)` | Sonnet | 10 contrats en < 10 min code | **Speed dev** |
| J8 | Bindings PyO3 | `Contract` opaque, import fluide | Sonnet | `import kontract` marche | Production |
| J9c | Batch pricing | 100+ contrats en < 500ms (`rayon` parallèle) | Sonnet | Portfolio pricing fast | **Price book** |
| J9 | Validation | vanilla, asian, KO, swaption (suite) | Sonnet | Tous prix tolérances | E2E |
| J10 | Release | CI, wheels, PyPI | Sonnet | Installable | Distribution |

## Phase 2 : Modèles avancés (semaines 3–8)

| # | Titre | Contenu | Modèle | Critère | Trader use |
|---|-------|---------|--------|---------|------------|
| J11 | Simulator trait | `Simulator` trait ; refactoriser `GBMSimulator` | **Opus** | GBM unchanged, J5+J6+J7 verts | Plugin system |
| J12 | Heston + Dupire | `HestonSimulator` (2D), `DupireSimulator` (vol locale) | Sonnet | Heston tol. 0.5% | **Stoch vol** |
| J13 | SABR + Merton | `SABRSimulator`, `MertonJumpSimulator` (Poisson) | Sonnet | 6 tests (3 SABR, 3 Merton) | Exotiques |
| J14 | Rough Bergomi | `RoughBergomiSimulator` (fBm) | **Opus** | Paths OK, convergence 5% | SOTA (optionnel) |
| J15 | Réduction variance | Antithétiques + contrôle ; CRN | Sonnet | σ² ÷ 2, prix identique | **Faster MC** |
| J16 | Quasi-MC (Sobol) | Sobol + Brownian bridge, O(1/N) | Sonnet | 1/N observable | **Better convergence** |
| J17 | Américaines (LSM) | Longstaff-Schwartz backward induction | **Opus** | US put vs CRR (0.5%) | Américaines (optionnel) |
| J18 | Multilevel MC | Hiérarchie Δt, coût O(ε^{-2}) | **Opus** | Économies mesurables | Extreme accuracy |

## Phase 2b : Calibration rapide (semaines 5–7)

| # | Titre | Contenu | Modèle | Critère | Trader use |
|---|-------|---------|--------|---------|------------|
| J21 | Lecteur données | yfinance/CSV → surface vol (Brent) | Sonnet | S&P 500 surface plausible | **Market data** |
| J21-fast | Calibration rapide | Trust region LM (< 1 sec, pas CMA-ES) | Sonnet | Heston/Dupire/SABR < 1 sec | **Real-time calib** |

## Phase 3 : Risk Manager (optionnel, après MVP)

| # | Titre | Contenu | Modèle | Critère | Risk use |
|---|-------|---------|--------|---------|-----------|
| J19 | Crank-Nicolson 1D | EDP BS 1D + PSOR américaines | **Opus** | Call EU vs BS (0.5%), US vs CRR | PDE pricing |
| J20 | ADI 2D | EDP 2D Heston ou corrélation 2 assets | **Opus** | Heston EDP ≈ MC (1%) | 2D pricing |
| J22 | Optimiseur complet | CMA-ES + trust region, contraintes | **Opus** | Round-trip < 1% params | Advanced calib |
| J23 | Backtesting | Prix historiques vs modèle, stability | Sonnet | Validation PnL réel | Model validation |

## Phase 4 : Extension future (après J22)

| # | Titre | Contenu | Modèle | Critère |
|---|-------|---------|--------|---------|
| J24 | Taux stochastiques | Vasicek/Hull-White pour contrats taux | Opus | Swaption vs analytique |
| J25 | FX simple | Corrélation spot/rate, multi-devise | Sonnet | Cross-currency options |

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
