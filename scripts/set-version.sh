#!/usr/bin/env bash
# Escreve a versao do app nos manifestos, a partir do nome de uma tag.
#
# A tag e a unica fonte da verdade da versao. O repositorio guarda
# `version = "0.0.0"` em `src-tauri/Cargo.toml` de proposito: quem publica uma
# release e o `release.yml`, que roda este script com `$GITHUB_REF_NAME` antes
# de chamar o bundler. Nao existe commit de bump pra esquecer, nem guard pra
# abortar a tag por divergencia — o valor e derivado, nao declarado.
#
#   ./scripts/set-version.sh v0.1.4     # tambem aceita 0.1.4
#
# Rodar isso a mao so faz sentido pra reproduzir um build de release
# localmente; o resultado nao deve ser commitado.
#
# Por que dois arquivos e nao quatro:
#   - `tauri.conf.json` nao declara `version`. Sem o campo, o bundler do
#     Tauri v2 usa `package.version` do Cargo.toml.
#   - `package.json` e `private: true` e nunca vai pro npm, entao nao precisa
#     de versao — e nada no build a le.
#   - `Cargo.lock` carrega a versao do proprio crate, entao acompanha.
set -euo pipefail

cd "$(dirname "$0")/.."

raw="${1:-}"
version="${raw#v}"
# `0.1.4-rc.1` -> `0.1.4`: o sufixo de pre-release existe somente na tag. O MSI
# do Windows exige `X.Y.Z` numerico e o bundler recusa qualquer outra coisa.
version="${version%%-*}"

if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "uso: $0 <vX.Y.Z[-sufixo]>" >&2
  exit 1
fi

# `sed -i` sem sufixo nao e portavel (o BSD exige o argumento), entao os
# reescritos usam perl. A flag `!$d` limita a substituicao a primeira
# ocorrencia: o `version = ` do [package] e seguido por dezenas de
# `version = "2"` de dependencia.
perl -pi -e 'if (!$d && s/^version = "[^"]+"/version = "'"$version"'"/) { $d = 1 }' \
  src-tauri/Cargo.toml

# `\r?` porque o runner Windows faz checkout com CRLF; sem isso o padrao nao
# casa e o lock sai intacto (o readback abaixo pegaria, mas depois do commit
# do erro num arquivo e nao no outro).
perl -0pi -e 's/(\nname = "abraxas"\r?\nversion = ")[^"]+(")/${1}'"$version"'${2}/' \
  src-tauri/Cargo.lock

# Readback: um padrao que deixou de casar (renomeacao do crate, reordenacao do
# manifesto) falharia silenciosamente e o instalador sairia como 0.0.0.
cargo_v=$(grep -m1 '^version = ' src-tauri/Cargo.toml | cut -d'"' -f2)
# awk, nao `grep -A1`, porque `abraxas-devtools` tambem casa o prefixo do nome.
lock_v=$(awk '/^name = "abraxas"\r?$/ { getline; print; exit }' src-tauri/Cargo.lock \
  | cut -d'"' -f2)

for pair in "Cargo.toml:$cargo_v" "Cargo.lock:$lock_v"; do
  if [[ "${pair#*:}" != "$version" ]]; then
    echo "::error::${pair%%:*} ficou em '${pair#*:}' em vez de $version — o padrao de substituicao nao casou." >&2
    exit 1
  fi
done

echo "versao $version escrita em src-tauri/Cargo.toml e src-tauri/Cargo.lock"
