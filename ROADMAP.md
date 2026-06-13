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

## Jalons

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

## Invariants à ne jamais violer

1. L'AST reste **sérialisable et pur** : aucune logique de pricing dedans.
2. Le pricing est **compositionnel** : le prix de `and(a, b)` est dérivé de
   ceux de `a` et `b`, jamais d'un cas spécial.
3. Les Greeks utilisent des **common random numbers** (même seed) pour réduire
   la variance du bump-and-reprice.
4. Toute primitive ajoutée à l'algèbre doit être couverte par un test de path.
