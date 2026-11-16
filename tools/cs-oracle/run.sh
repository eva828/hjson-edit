#!/usr/bin/env bash
# ビルド済みオラクルを実行する。使い方: run.sh <cases|assets|dir> <path>
# dotnet が PATH にあればそれを使い、無ければ nix で用意する。
set -euo pipefail
cd "$(dirname "$0")/../.."
. tools/cs-oracle/reference.env
REPO_ROOT="$(pwd)"
ORACLE="$REPO_ROOT/tools/cs-oracle/bin/Release/net10.0/Oracle.dll"

if command -v dotnet >/dev/null 2>&1; then
  export DOTNET_CLI_HOME="${DOTNET_CLI_HOME:-/tmp/dotnethome}"
  export NUGET_PACKAGES="${NUGET_PACKAGES:-/tmp/nuget}"
  export DOTNET_NOLOGO=1
  dotnet "$ORACLE" "$1" "$2"
else
  nix --extra-experimental-features 'nix-command flakes' --option sandbox false \
    shell "$DOTNET_ATTR" -c bash -c "
      set -euo pipefail
      export DOTNET_CLI_HOME=\${DOTNET_CLI_HOME:-/tmp/dotnethome}
      export NUGET_PACKAGES=\${NUGET_PACKAGES:-/tmp/nuget}
      export DOTNET_NOLOGO=1
      dotnet '$ORACLE' '$1' '$2'
    "
fi
