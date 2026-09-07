#!/usr/bin/env bash
# Guarda e devolve o estado local do Abraxas, pra gravar o app "do zero".
#
# Todo o estado do app mora em dois diretorios no macOS:
#
#   ~/Library/Application Support/abraxas.arthuradriano.com   (sqlite, models/, caches)
#   ~/Library/Logs/abraxas.arthuradriano.com                  (abraxas.log.*)
#
# `stash` move os dois pra um esconderijo; o proximo launch acha a maquina
# virgem e roda o onboarding de verdade — deteccao de hardware, catalogo,
# download, primeira conversa. `restore` devolve tudo e joga fora o que a
# gravacao criou. Nada e apagado sem confirmacao, e o esconderijo fica fora
# do repositorio.
#
#   ./scripts/demo-reset.sh stash      # antes de gravar
#   ./scripts/demo-reset.sh status     # onde esta o que
#   ./scripts/demo-reset.sh restore    # depois de gravar
#
# O app precisa estar fechado nos dois casos: o SQLite fica em modo WAL e
# mover o diretorio embaixo de um processo vivo corrompe o banco.
#
# Pra que a animacao de abertura so rode quando a gravacao ja estiver rolando,
# compile o app com VITE_DEMO_MODE=1 (ver src/lib/demo.ts).

set -euo pipefail

DATA="$HOME/Library/Application Support/abraxas.arthuradriano.com"
LOGS="$HOME/Library/Logs/abraxas.arthuradriano.com"
STASH="$HOME/.abraxas-demo-stash"

die() {
  echo "erro: $*" >&2
  exit 1
}

[[ "$(uname -s)" == "Darwin" ]] || die "este script e so pra macOS."

# Mover o diretorio de dados embaixo do app rodando corrompe o WAL do SQLite.
#
# O processo se chama `abraxas` minusculo — e o nome do binario, nao o do
# bundle (`Abraxas.app/Contents/MacOS/abraxas`). `pgrep -x Abraxas` nunca casa.
# O nome exato tambem pega o `tauri dev`, que roda o mesmo binario, e evita o
# falso positivo de um `pgrep -f` casando com a propria linha de comando.
assert_app_closed() {
  if pgrep -x abraxas >/dev/null 2>&1; then
    die "o Abraxas esta aberto. Feche o app (Cmd+Q) e rode de novo."
  fi
}

size_of() {
  [[ -e "$1" ]] && du -sh "$1" 2>/dev/null | cut -f1 || echo "-"
}

cmd_status() {
  echo "ao vivo:"
  printf '  %-10s %-6s %s\n' "dados" "$(size_of "$DATA")" "$DATA"
  printf '  %-10s %-6s %s\n' "logs" "$(size_of "$LOGS")" "$LOGS"
  echo "guardado:"
  if [[ -d "$STASH" ]]; then
    printf '  %-10s %-6s %s\n' "dados" "$(size_of "$STASH/data")" "$STASH/data"
    printf '  %-10s %-6s %s\n' "logs" "$(size_of "$STASH/logs")" "$STASH/logs"
  else
    echo "  (nada) $STASH"
  fi
}

cmd_stash() {
  assert_app_closed
  # Um esconderijo por vez: dois `stash` seguidos sobrescreveriam o real com o
  # da gravacao anterior, que e exatamente o dado que nao da pra recuperar.
  [[ -d "$STASH" ]] && die "ja existe um esconderijo em $STASH. Rode 'restore' antes."

  mkdir -p "$STASH"
  if [[ -d "$DATA" ]]; then
    mv "$DATA" "$STASH/data"
    echo "guardado: dados -> $STASH/data"
  else
    echo "nada a guardar: $DATA nao existe"
  fi
  if [[ -d "$LOGS" ]]; then
    mv "$LOGS" "$STASH/logs"
    echo "guardado: logs  -> $STASH/logs"
  fi
  echo
  echo "a maquina esta virgem. Abra o app e ele roda o onboarding."
}

cmd_restore() {
  assert_app_closed
  [[ -d "$STASH" ]] || die "nao ha esconderijo em $STASH."

  # O que a gravacao criou some; pode incluir um GGUF baixado ao vivo.
  if [[ -d "$DATA" || -d "$LOGS" ]]; then
    echo "a gravacao deixou pra tras:"
    [[ -d "$DATA" ]] && printf '  %-6s %s\n' "$(size_of "$DATA")" "$DATA"
    [[ -d "$LOGS" ]] && printf '  %-6s %s\n' "$(size_of "$LOGS")" "$LOGS"
    read -r -p "apagar e devolver o estado guardado? [s/N] " reply
    [[ "$reply" == "s" || "$reply" == "S" ]] || die "abortado; nada foi tocado."
    rm -rf "$DATA" "$LOGS"
  fi

  [[ -d "$STASH/data" ]] && mv "$STASH/data" "$DATA" && echo "devolvido: dados"
  [[ -d "$STASH/logs" ]] && mv "$STASH/logs" "$LOGS" && echo "devolvido: logs"
  rmdir "$STASH" 2>/dev/null || echo "aviso: $STASH nao ficou vazio; confira o que sobrou."
  echo
  echo "estado real de volta no lugar."
}

case "${1:-status}" in
  stash) cmd_stash ;;
  restore) cmd_restore ;;
  status) cmd_status ;;
  *) die "uso: $0 {stash|restore|status}" ;;
esac
