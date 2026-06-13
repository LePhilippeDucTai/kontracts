#!/bin/bash

# Skill : /jalon — orchestration des jalons kontract
# Usage: jalon [status|start|resume|done|stop]

set -euo pipefail

REPO_ROOT="${PWD}"
ROADMAP="${REPO_ROOT}/ROADMAP.md"
PROGRESS="${REPO_ROOT}/PROGRESS.md"

# Couleurs
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

function error() {
  echo -e "${RED}❌ Erreur: $1${NC}" >&2
  exit 1
}

function success() {
  echo -e "${GREEN}✅ $1${NC}"
}

function info() {
  echo -e "${BLUE}ℹ️  $1${NC}"
}

function warn() {
  echo -e "${YELLOW}⚠️  $1${NC}"
}

# Parse PROGRESS.md to find the first TODO/IN_PROGRESS jalon
function current_jalon() {
  local jalon=$(grep "^| J[0-9]" "$PROGRESS" | grep -E "TODO|IN_PROGRESS" | head -1 | awk '{print $2}' | tr -d '|')
  if [ -z "$jalon" ]; then
    # All done in this phase
    return 1
  fi
  echo "$jalon"
}

# Extract jalon info from ROADMAP.md
function jalon_info() {
  local j="$1"

  # Find the section for this jalon in ROADMAP
  # Grep for the line with "| J[number]" and extract titre, contenu, modèle, critère

  local line=$(grep "^| $j " "$ROADMAP")
  if [ -z "$line" ]; then
    error "Jalon $j non trouvé dans ROADMAP.md"
  fi

  # Parse the line (format: | # | Titre | Contenu | Modèle | Critère |)
  local titre=$(echo "$line" | awk -F'|' '{print $3}' | xargs)
  local contenu=$(echo "$line" | awk -F'|' '{print $4}' | xargs)
  local modele=$(echo "$line" | awk -F'|' '{print $5}' | xargs)
  local critere=$(echo "$line" | awk -F'|' '{print $6}' | xargs)

  # Find the phase
  local phase=""
  if [[ $j =~ ^J([0-9]+) ]]; then
    local num=${BASH_REMATCH[1]}
    if [ $num -le 10 ]; then
      phase="Phase 1 : MVP Trader (J1–J10)"
    elif [ $num -le 18 ] || [ "$j" == "J21" ] || [ "$j" == "J21-fast" ]; then
      phase="Phase 2 : Modèles avancés (J11–J21-fast)"
    else
      phase="Phase 3 : Risk Manager (J19–J23)"
    fi
  fi

  echo "=== Jalon actif: $j ==="
  echo ""
  echo "Titre:  $titre"
  echo "Modèle: $modele"
  echo "Phase:  $phase"
  echo ""
  echo "Contenu:"
  echo "  $contenu"
  echo ""
  echo "Critère de complétion:"
  echo "  $critere"
  echo ""
}

# Display full jalon context with dependencies
function display_context() {
  local j="$1"

  jalon_info "$j"

  # Check if depends on previous
  local prev_num=$((${j:1} - 1))
  local prev="J${prev_num}"
  if grep -q "^| $prev " "$ROADMAP" 2>/dev/null; then
    local prev_status=$(grep "^| $prev " "$PROGRESS" | awk '{print $(NF-1)}' | tr -d '|')
    if [ "$prev_status" != "DONE" ]; then
      warn "Dépendance bloquante: $prev est $prev_status (doit être DONE)"
    else
      info "✓ Dépendance $prev : DONE"
    fi
  fi

  echo ""
  echo "Prochaines étapes:"
  echo "  1. Lire ROADMAP.md et PROGRESS.md pour le contexte"
  echo "  2. Lancer avec : jalon start"
  echo "  3. Implémenter jusqu'au critère de complétion"
  echo "  4. Faire passer les tests : cargo test"
  echo "  5. Committer et tagger: git tag $j-<slug>"
  echo "  6. Marquer DONE : jalon done"
  echo ""
}

# Start implementation of a jalon
function start_jalon() {
  local j="$1"

  # Mark as IN_PROGRESS in PROGRESS.md (only for this jalon)
  if grep -q "^| $j " "$PROGRESS"; then
    # Use sed to replace status for this specific jalon line
    if [[ "$OSTYPE" == "darwin"* ]]; then
      sed -i '' "/^| $j /s/| TODO |/| IN_PROGRESS |/" "$PROGRESS"
    else
      sed -i "/^| $j /s/| TODO |/| IN_PROGRESS |/" "$PROGRESS"
    fi
  fi

  clear
  info "=== Démarrage du jalon $j ==="
  echo ""
  display_context "$j"

  success "Jalon $j marqué comme IN_PROGRESS dans PROGRESS.md"
  echo ""
  echo "Commande pour reprendre:"
  echo "  jalon resume"
  echo ""
  echo "Commande pour marquer DONE (après commit):"
  echo "  jalon done"
}

# Mark jalon as DONE
function mark_done() {
  local j="$1"

  # Check git status - must have recent commit with tag
  if ! git describe --tags --exact-match 2>/dev/null | grep -q "^$j"; then
    warn "Pas de tag $j trouvé. Vérifiez que vous avez commité avec le tag approprié:"
    echo "  git tag $j-<slug>"
    return 1
  fi

  # Update PROGRESS.md (mark this jalon as DONE)
  if [[ "$OSTYPE" == "darwin"* ]]; then
    sed -i '' "/^| $j /s/| IN_PROGRESS |/| DONE |/" "$PROGRESS"
  else
    sed -i "/^| $j /s/| IN_PROGRESS |/| DONE |/" "$PROGRESS"
  fi

  success "Jalon $j marqué comme DONE dans PROGRESS.md"

  # Try to find next jalon
  if next=$(current_jalon); then
    info "Prochain jalon: $next"
    echo ""
    echo "Pour continuer: jalon start"
  else
    success "🎉 Tous les jalons de cette phase sont DONE!"
  fi
}

# Resume current jalon
function resume_jalon() {
  local j
  if j=$(current_jalon); then
    display_context "$j"
  else
    warn "Aucun jalon en cours. Tous les jalons de cette phase sont DONE!"
  fi
}

# Status command
function status() {
  echo "=== État du projet kontract ==="
  echo ""

  # Count done/todo
  local total=$(grep -c "^| J[0-9]" "$PROGRESS" || true)
  local done=$(grep -c "| DONE |" "$PROGRESS" || true)

  echo "Progression: $done / $total jalons DONE"
  echo ""

  # Show current jalon
  if jalon=$(current_jalon); then
    info "Jalon actif: $jalon"
    jalon_info "$jalon"
  else
    success "Phase terminée! 🎉"
  fi
}

# Main command dispatcher
function main() {
  case "${1:-status}" in
    status|st)
      status
      ;;
    start|s)
      if jalon=$(current_jalon); then
        start_jalon "$jalon"
      else
        warn "Aucun jalon TODO. Tous sont DONE ou IN_PROGRESS."
      fi
      ;;
    resume|r)
      resume_jalon
      ;;
    done|d)
      if jalon=$(current_jalon); then
        mark_done "$jalon"
      else
        warn "Aucun jalon en cours."
      fi
      ;;
    stop)
      info "Arrêt de la session. État sauvegardé dans PROGRESS.md"
      ;;
    help|h|-h|--help)
      echo "Skill: /jalon — orchestration des jalons kontract"
      echo ""
      echo "Usage: jalon [commande]"
      echo ""
      echo "Commandes:"
      echo "  status    Affiche l'état du jalon actuel"
      echo "  start     Démarre l'implémentation du jalon actuel"
      echo "  resume    Reprend le jalon interrompu"
      echo "  done      Marque le jalon comme complété (après commit + tag)"
      echo "  stop      Arrête la session"
      echo "  help      Affiche cette aide"
      ;;
    *)
      error "Commande inconnue: $1. Tapez 'jalon help' pour l'aide."
      ;;
  esac
}

# Run main
main "$@"
