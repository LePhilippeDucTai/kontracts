#!/bin/bash

# Hook post-commit: marque jalon DONE si tests verts + tag de jalon détecté
# Exécuté automatiquement après chaque commit

set -euo pipefail

PROGRESS="${PWD}/PROGRESS.md"

# Couleurs
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

function info()    { echo -e "${BLUE}→${NC} $1"; }
function success() { echo -e "${GREEN}✓${NC} $1"; }

# Détecte si le commit courant a un tag de jalon (j17-*, j18-*, etc.)
function detect_jalon_tag() {
  local tag
  tag=$(git describe --tags --exact-match 2>/dev/null || true)

  if [[ $tag =~ ^(j[0-9]+[a-z]*)-.*$ ]]; then
    local j="${BASH_REMATCH[1]}"
    # Normaliser j9c → J9c, j17 → J17
    j=$(echo "$j" | sed 's/^j/J/')
    echo "$j"
  fi
}

# Vérifie si un jalon est IN_PROGRESS
function jalon_is_in_progress() {
  local j="$1"
  grep "^| $j " "$PROGRESS" 2>/dev/null | grep -q "| IN_PROGRESS |"
}

# Marque un jalon comme DONE
function mark_jalon_done() {
  local j="$1"
  sed -i "/^| $j /s/| IN_PROGRESS |/| DONE |/" "$PROGRESS"
  success "$j marqué DONE automatiquement (tests verts + tag détecté)"
}

# Main
j=$(detect_jalon_tag)
if [ -z "$j" ]; then
  exit 0  # Pas de tag de jalon, rien à faire
fi

info "Détecté tag de jalon : $j"

# Vérifier que le jalon est IN_PROGRESS
if ! jalon_is_in_progress "$j"; then
  info "$j n'est pas IN_PROGRESS (ou inexistant) — ignorer"
  exit 0
fi

# Vérifier que les tests passent (optionnel, peut être long)
# On peut le désactiver en commentant la ligne suivante si test trop long
if cargo test --release -q 2>/dev/null; then
  mark_jalon_done "$j"

  # Committer la mise à jour de PROGRESS.md
  git add PROGRESS.md
  git commit --no-verify -m "Auto-DONE: $j (tests ✓)" 2>/dev/null || true

  info "Jalon $j prêt pour le suivant : /jalon status"
else
  info "Tests échouent — $j reste IN_PROGRESS"
fi
