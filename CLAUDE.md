# CLAUDE.md — `kontract`

## Ce qu'est ce projet

Une **algèbre des contrats financiers** : un DSL compositionnel (combinateurs
primitifs à la Peyton Jones) qui compile vers des payoffs évaluables par
Monte-Carlo. Cœur en Rust, exposé à Python via PyO3/maturin, publiable sur PyPI.

Lis `ROADMAP.md` avant toute action. Lis `PROGRESS.md` pour savoir où on en est.

## Principe architectural central

Trois couches strictement séparées :

```
DSL / AST  (pur, sérialisable)
    │  ── compile ──>
IR / timeline d'événements  (plan de calcul)
    │  ── simulate + evaluate ──>
Pricer Monte-Carlo  (paths, agrégation, discount)
```

L'AST ne contient **aucune** logique numérique. Le pricer ne connaît **aucun**
produit financier nommé : il ne sait évaluer que les combinateurs primitifs.
Un "call européen" n'existe pas dans le moteur — c'est une expression du DSL.

## Combinateurs primitifs (ne pas en ajouter sans mise à jour de ROADMAP)

```
zero | one(ccy) | give(c) | and(c1,c2) | or(c1,c2)
| scale(obs, c) | when(cond, c) | anytime(cond, c) | until(cond, c)
```

## Conventions de code

- Rust 2021, `cargo fmt` + `cargo clippy -- -D warnings` avant chaque commit.
- Pas de `unwrap()` hors tests ; propager via `Result<_, KontractError>`.
- Parallélisme MC via `rayon`, jamais de threads manuels.
- Arrays via `ndarray` ; échange Python zéro-copie via le buffer protocol (numpy).
- Tests : un fichier par primitive dans `tests/`, produits composés dans
  `tests/products/`.

## Discipline de jalon (IMPORTANT)

À chaque exécution de `/loop` :

1. Lire `PROGRESS.md`, identifier le **premier jalon non `DONE`**.
2. Utiliser le **modèle indiqué** pour ce jalon dans `ROADMAP.md`.
3. Implémenter UNIQUEMENT ce jalon. Ne pas anticiper les suivants.
4. Écrire/faire passer les tests jusqu'au critère de complétion.
5. `cargo fmt && cargo clippy && cargo test` doivent être verts.
6. Pour un jalon **Opus** : lancer une revue Opus séparée (voir SKILL) avant commit.
7. Commit + tag `jX-<slug>`, puis passer le jalon à `DONE` dans `PROGRESS.md`
   avec un court résumé des décisions prises.
8. S'arrêter. Une exécution de `/loop` = un jalon.

## Décisions déjà prises (ne pas rediscuter)

- Modèle de diffusion initial : GBM. Heston/SABR viendront après J10 comme
  simulateurs branchables, sans toucher au pricer.
- `f64` partout (pas de génériques sur le scalaire au début).
- Discount déterministe (taux constant) jusqu'à J9 ; taux stochastique = extension.

## Ce qu'il ne faut PAS faire

- Ne pas introduire de cas spécial par produit dans le pricer.
- Ne pas mettre de logique de pricing dans l'AST.
- Ne pas sauter la revue Opus sur J4 et J6.
- Ne pas committer avec des warnings clippy ou des tests rouges.
