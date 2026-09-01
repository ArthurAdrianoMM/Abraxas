#!/usr/bin/env bash
# Bumpa a versao do app nos quatro lugares que a declaram e cria a tag.
#
# O job `guard` do release.yml aborta a tag se package.json, tauri.conf.json ou
# Cargo.toml divergirem dela — foi assim que a v0.1.1 morreu em 11s. Bumpar os
# arquivos a mao e esquecer um deles e o modo de falha default, entao o bump e
# um comando so.
#
#   ./scripts/bump-version.sh 0.1.1          # edita, commita e cria a tag local
#   ./scripts/bump-version.sh 0.1.1 --no-tag # so edita e commita
#
# O push da tag (que dispara o release) fica com quem roda: `git push origin
# main --follow-tags`.
set -euo pipefail

cd "$(dirname "$0")/.."

version="${1:-}"
tag_it=1
[[ "${2:-}" == "--no-tag" ]] && tag_it=0

if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "uso: $0 <X.Y.Z> [--no-tag]" >&2
  echo "  a versao nos manifestos e sempre X.Y.Z; sufixos de pre-release" >&2
  echo "  (-rc.1) existem somente na tag, e o guard os ignora." >&2
  exit 1
fi

if [[ -n "$(git status --porcelain)" ]]; then
  echo "erro: worktree sujo — commite ou stashe antes de bumpar." >&2
  exit 1
fi

# Checado antes de mexer nos arquivos: descobrir a colisao depois do commit
# deixaria o bump commitado e a tag nao, um estado meio-feito confuso de
# desfazer.
if (( tag_it )) && git rev-parse -q --verify "refs/tags/v$version" >/dev/null; then
  echo "erro: a tag v$version ja existe. Se ela aponta pro commit errado:" >&2
  echo "  git tag -d v$version && git push origin :refs/tags/v$version" >&2
  echo "  (apague tambem o release em rascunho, se houver)" >&2
  exit 1
fi

# `sed -i` sem sufixo nao e portavel (o BSD exige o argumento), entao os
# reescritos usam perl. A flag `!$d` limita cada substituicao a primeira
# ocorrencia: em Cargo.toml o `version = ` do [package] e seguido por dezenas
# de `version = "2"` de dependencia. Um perl por arquivo, porque `$d`
# sobreviveria entre os arquivos de uma mesma invocacao e o segundo arquivo
# sairia intacto.
for f in package.json src-tauri/tauri.conf.json; do
  perl -pi -e 'if (!$d && s/^(\s*"version"\s*:\s*")[^"]+(")/${1}'"$version"'${2}/) { $d = 1 }' "$f"
done
perl -pi -e 'if (!$d && s/^version = "[^"]+"/version = "'"$version"'"/) { $d = 1 }' \
  src-tauri/Cargo.toml

# Cargo.lock carrega a versao do proprio crate. `cargo build` a corrigiria
# sozinho, mas deixar o lock desatualizado no commit da tag significa que o
# primeiro build do release mexe num arquivo versionado — e `--locked`, se um
# dia entrar no CI, falharia. `--offline` mantem o bump sem rede.
cargo update --offline --package abraxas --manifest-path src-tauri/Cargo.toml >/dev/null 2>&1 \
  || perl -0pi -e 's/(\nname = "abraxas"\nversion = ")[^"]+(")/${1}'"$version"'${2}/' src-tauri/Cargo.lock

# Confere o resultado com a mesma leitura que o guard faz, pra o erro aparecer
# aqui e nao 11s depois do push da tag.
pkg=$(node -p "require('./package.json').version")
conf=$(node -p "require('./src-tauri/tauri.conf.json').version")
cargo=$(grep -m1 '^version = ' src-tauri/Cargo.toml | cut -d'"' -f2)
lock=$(grep -A1 '^name = "abraxas"$' src-tauri/Cargo.lock | grep -m1 '^version = ' | cut -d'"' -f2)

for pair in "package.json:$pkg" "tauri.conf.json:$conf" "Cargo.toml:$cargo" "Cargo.lock:$lock"; do
  if [[ "${pair#*:}" != "$version" ]]; then
    echo "erro: ${pair%%:*} ficou em ${pair#*:} em vez de $version" >&2
    exit 1
  fi
done

git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -q -m "chore(release): v$version"
echo "commit chore(release): v$version criado"

if (( tag_it )); then
  git tag -a "v$version" -m "Abraxas v$version"
  echo "tag v$version criada. Publique com:"
  echo "  git push origin $(git rev-parse --abbrev-ref HEAD) --follow-tags"
fi
