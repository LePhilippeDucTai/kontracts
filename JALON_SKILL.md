# Skill : `/jalon` — Orchestration des jalons

**Skill pour lancer et gérer l'implémentation des jalons du projet `kontract` de manière fluide et reproductible.**

## Utilisation rapide

```bash
# Voir l'état du jalon actuel
jalon status

# Démarrer l'implémentation du jalon actuel
jalon start

# Reprendre après interruption
jalon resume

# Marquer complété (après commit + git tag)
jalon done

# Affiche cette aide
jalon help
```

## Workflow complet pour un jalon

```bash
# 1. Vérifier l'état
jalon status

# Affiche :
#  === État du projet kontract ===
#  Progression: 0 / 30 jalons DONE
#  ℹ️  Jalon actif: J1
#  === Jalon actif: J1 ===
#  Titre:  AST
#  Modèle: Sonnet
#  ...

# 2. Démarrer la mise en œuvre
jalon start

# Affiche le contexte complet + étapes

# 3. Implémenter jusqu'au critère de complétion
# → cargo fmt && cargo clippy && cargo test
# → commit : git add ... && git commit -m "..."
# → tag : git tag J1-<slug> 

# 4. Marquer comme DONE (met à jour PROGRESS.md)
jalon done

# Affiche le prochain jalon ou "🎉 Tous les jalons de cette phase sont DONE!"
```

## Commandes détaillées

### `jalon status` (ou `jalon st`)

Affiche l'état actuel :
- Progression globale (X/30 jalons DONE)
- Jalon actif (premier TODO ou IN_PROGRESS)
- Titre, modèle, phase, contenu, critères

**Exemple** :
```
=== État du projet kontract ===
Progression: 5 / 30 jalons DONE

ℹ️  Jalon actif: J6
=== Jalon actif: J6 ===
Titre:  Barrières
Modèle: Opus
Phase:  Phase 1 : MVP Trader (J1–J10)
Contenu: `until`, `anytime` (activation par path)
Critère de complétion: KO call vs analytique (2%)
```

### `jalon start` (ou `jalon s`)

Lance l'implémentation du jalon actuel :
- Affiche le contexte complet
- Marque le jalon `TODO` → `IN_PROGRESS` dans PROGRESS.md
- Affiche les étapes suivantes

**Output** :
```
ℹ️  === Démarrage du jalon J1 ===

=== Jalon actif: J1 ===
Titre:  AST
[contexte complet]

Prochaines étapes:
  1. Lire ROADMAP.md et PROGRESS.md pour le contexte
  2. Implémenter jusqu'au critère de complétion
  3. Faire passer les tests : cargo test
  4. Committer et tagger: git tag J1-<slug>
  5. Marquer DONE : jalon done

✅ Jalon J1 marqué comme IN_PROGRESS dans PROGRESS.md
```

### `jalon resume` (ou `jalon r`)

Reprend le jalon interrompu (affiche le contexte sans modifier l'état).

Utile si vous avez fermé la session ou quitté.

### `jalon done` (ou `jalon d`)

Marque le jalon comme `DONE` dans PROGRESS.md.

**Prérequis** :
- Vous devez avoir commité votre travail
- Vous devez avoir créé un tag git `JX-<slug>`

**Exemple** :
```bash
# Après commit
git tag J1-ast-serde

# Marquer DONE
jalon done

# Output:
# ✅ Jalon J1 marqué comme DONE dans PROGRESS.md
# ℹ️  Prochain jalon: J2
# Pour continuer: jalon start
```

### `jalon stop`

Arrête la session (simple message, l'état est sauvegardé dans PROGRESS.md).

### `jalon help` (ou `jalon h`)

Affiche l'aide du skill.

---

## Architecture du skill

**Fichier** : `./.claude/skills/jalon.sh`

Le script :
1. Lit PROGRESS.md pour détecter le jalon actuel (premier TODO/IN_PROGRESS)
2. Extrait les infos du jalon depuis ROADMAP.md
3. Exécute les commandes (status, start, resume, done, stop)
4. Met à jour PROGRESS.md de manière atomique (sed spécifique à la ligne du jalon)

### Parsing

- **PROGRESS.md** : format tableau markdown standard
- **ROADMAP.md** : format tableau markdown standard

Le parsing est **robuste** (tolère des variations mineures de format).

---

## Workflow typique : jour 1 → 3 semaines

**Jour 1, matin**
```bash
jalon status      # J1 est actif
jalon start       # Démarrer J1 (AST)
```
→ Implémenter AST jusqu'au critère
→ Tests verts, commit
```bash
git tag J1-ast-serde
jalon done        # J1 → DONE
# Output: Prochain jalon: J2
```

**Jour 1, après-midi**
```bash
jalon start       # Démarrer J2 (Observables)
```
→ Implémenter Observables
→ Tests verts, commit, tag
```bash
git tag J2-observables
jalon done        # J2 → DONE, J3 actif
```

**Répéter pour J3, J4, ..., J10** → ~2–3 semaines pour MVP Trader ✅

---

## Intégration avec d'autres outils

### Avec `/loop`

Le skill `/jalon` peut être appelé avant `/loop` pour voir le contexte :
```bash
jalon status
# ...affiche le jalon...
# Ensuite, faire : /loop (ou lancer Claude manuellement)
```

### Avec Git

Après chaque commit :
```bash
git commit -m "Implement J5 pricer basic"
git tag J5-pricer-basic
jalon done
# Automatically: PROGRESS.md updated, next milestone displayed
```

### Avec tests

```bash
cargo test
# Si tous les tests passent:
git commit -m "..."
git tag JX-<slug>
jalon done
```

---

## FAQ

**Q: Que se passe-t-il si je ferme la session en cours de jalon?**
A: Pas de problème. PROGRESS.md enregistre l'état (IN_PROGRESS). 
   Relancez `jalon resume` pour reprendre où vous vous étiez arrêté.

**Q: Puis-je sauter un jalon?**
A: Non. Le script impose l'ordre strict (J1 → J2 → ... → J10 → J11 → ...).
   Cet ordre existe pour des raisons architecturales (dépendances).

**Q: Que faire si un jalon me prend plus de temps que prévu?**
A: Pas de problème. Le jalon reste `IN_PROGRESS` jusqu'à ce que vous fassiez `jalon done`.
   Vous pouvez le reprendre à tout moment avec `jalon resume`.

**Q: Comment changer le statut d'un jalon manuellement?**
A: Éditez PROGRESS.md directement (format : `| Jx | titre | DONE |`).
   Mais le script `/jalon` est recommandé pour la cohérence.

---

## Prochaines étapes

1. Exécuter `/jalon status` pour voir le contexte de J1
2. Exécuter `/jalon start` pour démarrer l'implémentation
3. Implémenter jusqu'au critère de complétion (voir ROADMAP.md)
4. Commit + tag + `jalon done` pour progresser

Bonne chance ! 🚀
