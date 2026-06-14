#!/bin/bash

# Skill : /jalon — orchestration des jalons kontract
# Usage: jalon [status|start|resume|done|stop|help]
#
# Lit PROGRESS.md pour l'état, aide Claude à implémenter le jalon suivant.

set -euo pipefail

REPO_ROOT="${PWD}"
PROGRESS="${REPO_ROOT}/PROGRESS.md"

# Couleurs (désactivées si pas de terminal interactif)
if [ -t 1 ]; then
  RED='\033[0;31m'
  GREEN='\033[0;32m'
  BLUE='\033[0;34m'
  YELLOW='\033[1;33m'
  BOLD='\033[1m'
  NC='\033[0m'
else
  RED='' GREEN='' BLUE='' YELLOW='' BOLD='' NC=''
fi

function error()   { echo -e "${RED}Erreur: $1${NC}" >&2; exit 1; }
function success() { echo -e "${GREEN}✓ $1${NC}"; }
function info()    { echo -e "${BLUE}→ $1${NC}"; }
function warn()    { echo -e "${YELLOW}⚠ $1${NC}"; }
function header()  { echo -e "${BOLD}$1${NC}"; }

# Parse PROGRESS.md — premier jalon TODO ou IN_PROGRESS
function current_jalon() {
  grep "^| J" "$PROGRESS" \
    | grep -E "\| (TODO|IN_PROGRESS) \|" \
    | head -1 \
    | awk -F'|' '{print $2}' \
    | tr -d ' '
}

# Titre du jalon depuis PROGRESS.md
function jalon_titre() {
  local j="$1"
  grep "^| $j " "$PROGRESS" | awk -F'|' '{print $3}' | tr -d ' '
}

# Statut du jalon depuis PROGRESS.md
function jalon_statut() {
  local j="$1"
  grep "^| $j " "$PROGRESS" | awk -F'|' '{print $4}' | tr -d ' '
}

# Résumé des décisions depuis PROGRESS.md
function jalon_resume() {
  local j="$1"
  grep "^| $j " "$PROGRESS" | awk -F'|' '{print $6}'
}

# Affiche l'état complet du projet
function cmd_status() {
  header "=== État du projet kontract ==="
  echo ""

  local total done todo in_progress
  total=$(grep -c "^| J" "$PROGRESS" || true)
  done=$(grep "^| J" "$PROGRESS" | grep -c "| DONE |" || true)
  todo=$(grep "^| J" "$PROGRESS" | grep -c "| TODO |" || true)
  in_progress=$(grep "^| J" "$PROGRESS" | grep -c "| IN_PROGRESS |" || true)

  echo "Progression : ${done} / ${total} DONE | ${in_progress} IN_PROGRESS | ${todo} TODO"
  echo ""

  # Tableau des phases
  header "Phase 1 : MVP Trader"
  grep "^| J[1-9][0b]* |" "$PROGRESS" | grep "^| J[1-9] \|" | while IFS= read -r line; do
    local j statut titre
    j=$(echo "$line" | awk -F'|' '{print $2}' | tr -d ' ')
    statut=$(echo "$line" | awk -F'|' '{print $4}' | tr -d ' ')
    titre=$(echo "$line" | awk -F'|' '{print $3}' | xargs)
    case "$statut" in
      DONE)        echo -e "  ${GREEN}✓${NC} $j — $titre" ;;
      IN_PROGRESS) echo -e "  ${YELLOW}▶${NC} $j — $titre  ← ACTIF" ;;
      TODO)        echo -e "    $j — $titre" ;;
    esac
  done

  echo ""
  header "Phase 2 : Modèles avancés"
  grep "^| J1[1-9] \||^| J2[0-9] \||^| J21-" "$PROGRESS" 2>/dev/null | while IFS= read -r line; do
    local j statut titre
    j=$(echo "$line" | awk -F'|' '{print $2}' | tr -d ' ')
    statut=$(echo "$line" | awk -F'|' '{print $4}' | tr -d ' ')
    titre=$(echo "$line" | awk -F'|' '{print $3}' | xargs)
    case "$statut" in
      DONE)        echo -e "  ${GREEN}✓${NC} $j — $titre" ;;
      IN_PROGRESS) echo -e "  ${YELLOW}▶${NC} $j — $titre  ← ACTIF" ;;
      TODO)        echo -e "    $j — $titre" ;;
    esac
  done

  echo ""
  local j
  j=$(current_jalon)
  if [ -n "$j" ]; then
    info "Prochain jalon actif : $j ($(jalon_titre "$j"))"
    echo "  → Lancer : /jalon start"
  else
    success "Tous les jalons sont DONE ! 🎉"
  fi
}

# Affiche le contexte détaillé d'un jalon spécifique
function cmd_info() {
  local j="${1:-$(current_jalon)}"
  [ -z "$j" ] && { success "Tous les jalons sont DONE."; return; }

  local titre statut resume
  titre=$(jalon_titre "$j")
  statut=$(jalon_statut "$j")
  resume=$(jalon_resume "$j")

  header "=== Jalon : $j — $titre ==="
  echo "Statut : $statut"
  echo ""

  if [ -n "$resume" ] && [ "$resume" != " " ]; then
    header "Décisions prises :"
    echo "$resume" | fold -s -w 80
    echo ""
  fi

  # Affiche plan depuis le fichier plan si disponible
  local plan_file
  plan_file=$(ls "$HOME/.claude/plans/"*-*.md 2>/dev/null | head -1 || true)
  if [ -n "$plan_file" ]; then
    local section
    section=$(grep -A 30 "^\*\*$j " "$plan_file" 2>/dev/null | head -20 || true)
    if [ -n "$section" ]; then
      header "Plan d'implémentation :"
      echo "$section"
      echo ""
    fi
  fi

  header "Prochaines étapes :"
  echo "  1. Lire PROGRESS.md et CLAUDE.md pour le contexte"
  echo "  2. Implémenter jusqu'au critère de complétion"
  echo "  3. cargo fmt && cargo clippy && cargo test --release"
  echo "  4. git tag $j-<slug> && git push"
  echo "  5. /jalon done"
}

# Démarre un jalon (TODO → IN_PROGRESS)
function cmd_start() {
  local j
  j=$(current_jalon)
  [ -z "$j" ] && { success "Tous les jalons sont DONE."; return; }

  local statut
  statut=$(jalon_statut "$j")

  if [ "$statut" = "IN_PROGRESS" ]; then
    warn "$j est déjà IN_PROGRESS"
    cmd_info "$j"
    return
  fi

  # Passage TODO → IN_PROGRESS
  sed -i "/^| $j /s/| TODO |/| IN_PROGRESS |/" "$PROGRESS"

  success "$j marqué IN_PROGRESS"
  echo ""
  cmd_info "$j"
}

# Reprend le jalon courant (alias de info sur IN_PROGRESS)
function cmd_resume() {
  local j
  j=$(current_jalon)
  [ -z "$j" ] && { success "Tous les jalons sont DONE."; return; }
  cmd_info "$j"
}

# Marque le jalon courant comme DONE
function cmd_done() {
  local j
  j=$(current_jalon)
  [ -z "$j" ] && { success "Tous les jalons sont DONE."; return; }

  local statut
  statut=$(jalon_statut "$j")
  if [ "$statut" != "IN_PROGRESS" ]; then
    warn "$j n'est pas IN_PROGRESS (statut : $statut). Faites d'abord /jalon start."
    return 1
  fi

  # Vérifications pré-done
  if ! cargo test --release -q 2>/dev/null; then
    error "Les tests Rust échouent. Corrigez avant de marquer DONE."
  fi

  # Passage IN_PROGRESS → DONE
  sed -i "/^| $j /s/| IN_PROGRESS |/| DONE |/" "$PROGRESS"

  success "$j marqué DONE dans PROGRESS.md"

  # Prochain jalon
  local next
  next=$(current_jalon)
  if [ -n "$next" ]; then
    info "Prochain jalon : $next — $(jalon_titre "$next")"
    echo "  → /jalon start"
  else
    success "Phase terminée ! 🎉"
  fi
}

# Affiche l'aide
function cmd_help() {
  header "Skill /jalon — orchestration des jalons kontract"
  echo ""
  echo "  /jalon status   Progression globale + jalon actif"
  echo "  /jalon start    Démarre le jalon suivant (TODO → IN_PROGRESS)"
  echo "  /jalon resume   Affiche le contexte du jalon courant"
  echo "  /jalon info     Idem (alias)"
  echo "  /jalon done     Marque le jalon DONE (après tests + commit)"
  echo "  /jalon stop     Arrête proprement la session"
  echo "  /jalon help     Affiche cette aide"
  echo ""
  echo "Workflow type :"
  echo "  /jalon status → /jalon start → [impl + tests] → /jalon done → (répéter)"
}

# Dispatcher principal
case "${1:-status}" in
  status|st)          cmd_status ;;
  start|s)            cmd_start ;;
  resume|r)           cmd_resume ;;
  info|i)             cmd_info "${2:-}" ;;
  done|d)             cmd_done ;;
  stop)               info "Session arrêtée. État sauvegardé dans PROGRESS.md." ;;
  help|h|-h|--help)   cmd_help ;;
  *)                  error "Commande inconnue : $1. Tapez '/jalon help'." ;;
esac
