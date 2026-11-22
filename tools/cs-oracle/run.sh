#!/usr/bin/env bash
# ビルド済みオラクルを実行する。使い方: run.sh <cases|assets|dir> <path>
#
# コンテナなど /run/user/<uid> が無い環境では、dotnet の初回起動が書き込み可能な
# ランタイムディレクトリを必要とする。次のように用意してから実行する:
#   mkdir -p /tmp/xdg && chmod 700 /tmp/xdg
#   XDG_RUNTIME_DIR=/tmp/xdg DOTNET_EnableDiagnostics=0 tools/corpus_diff.sh
set -euo pipefail
cd "$(dirname "$0")/../.."
. tools/cs-oracle/reference.env
REPO_ROOT="$(pwd)"
nix --extra-experimental-features 'nix-command flakes' --option sandbox false \
  shell "$DOTNET_ATTR" -c bash -c "
    set -euo pipefail
    export DOTNET_CLI_HOME=\${DOTNET_CLI_HOME:-/tmp/dotnethome}
    export NUGET_PACKAGES=\${NUGET_PACKAGES:-/tmp/nuget}
    export DOTNET_NOLOGO=1
    dotnet '$REPO_ROOT/tools/cs-oracle/bin/Release/net10.0/Oracle.dll' '$1' '$2'
  "
