#!/usr/bin/env bash
# hjson-cs を固定コミットで取得し、オラクルをビルドする。
# 標準出力の最終行に hjson-cs の作業ディレクトリを出す。
# dotnet が PATH にあればそれを使い、無ければ nix で用意する。
set -euo pipefail
cd "$(dirname "$0")/../.."
. tools/cs-oracle/reference.env

WORK="${HJSON_CS_WORK:-/tmp/hjson-cs-fixtures}"
[[ "$WORK" = /* ]] || WORK="$(pwd)/$WORK"

if [ ! -d "$WORK/.git" ]; then
  mkdir -p "$WORK"
  git -C "$WORK" init -q
  git -C "$WORK" remote add origin "$HJSON_CS_REPO"
fi
git -C "$WORK" fetch -q --depth 1 origin "$HJSON_CS_COMMIT"
git -C "$WORK" checkout -q FETCH_HEAD

REPO_ROOT="$(pwd)"

if command -v dotnet >/dev/null 2>&1; then
  export DOTNET_CLI_HOME="${DOTNET_CLI_HOME:-/tmp/dotnethome}"
  export NUGET_PACKAGES="${NUGET_PACKAGES:-/tmp/nuget}"
  export DOTNET_NOLOGO=1
  cd "$REPO_ROOT/tools/cs-oracle"
  dotnet build -c Release --nologo -v q -p:HjsonCsDir="$WORK" >&2
else
  nix --extra-experimental-features 'nix-command flakes' --option sandbox false \
    shell "$DOTNET_ATTR" -c bash -c "
      set -euo pipefail
      export DOTNET_CLI_HOME=\${DOTNET_CLI_HOME:-/tmp/dotnethome}
      export NUGET_PACKAGES=\${NUGET_PACKAGES:-/tmp/nuget}
      export DOTNET_NOLOGO=1
      cd '$REPO_ROOT/tools/cs-oracle'
      dotnet build -c Release --nologo -v q -p:HjsonCsDir='$WORK'
    " >&2
fi

echo "$WORK"
