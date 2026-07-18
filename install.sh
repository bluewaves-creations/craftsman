#!/bin/sh
# install.sh — team-local installer for the craftsman CLI.
#
# Order of preference:
#   1. a GitHub Release binary you already downloaded (place it beside
#      this script as ./craftsman, or as an extracted ./craftsman-*/ dir)
#   2. cargo install --path cli   (builds from this checkout)
# Then runs `craftsman setup` to install the bundled six skills.
#
# No network access here: download the release artifact yourself (or let
# cargo compile). POSIX sh, no bashisms.
set -eu

REPO_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
BIN_DIR="${CRAFTSMAN_BIN_DIR:-$HOME/.local/bin}"

say() { printf '%s\n' "$*" >&2; }

find_release_binary() {
    for cand in "$REPO_DIR/craftsman" "$REPO_DIR"/craftsman-*/craftsman; do
        if [ -f "$cand" ] && [ -x "$cand" ]; then
            printf '%s\n' "$cand"
            return 0
        fi
    done
    return 1
}

install_release() {
    src=$1
    mkdir -p "$BIN_DIR"
    cp "$src" "$BIN_DIR/craftsman"
    chmod +x "$BIN_DIR/craftsman"
    say "installed release binary -> $BIN_DIR/craftsman"
    case ":$PATH:" in
        *":$BIN_DIR:"*) ;;
        *) say "note: $BIN_DIR is not on PATH — add it to your shell profile" ;;
    esac
    CRAFTSMAN="$BIN_DIR/craftsman"
}

install_from_source() {
    if ! command -v cargo >/dev/null 2>&1; then
        say "error: no release binary found and cargo is not installed."
        say "  either download the craftsman release artifact for your"
        say "  platform next to this script, or install rust (rustup.rs)"
        say "  and re-run."
        exit 1
    fi
    say "no release binary found — building with cargo (a few minutes)…"
    cargo install --path "$REPO_DIR/cli" --locked
    CRAFTSMAN="$(command -v craftsman || printf '%s' "$HOME/.cargo/bin/craftsman")"
    say "installed via cargo -> $CRAFTSMAN"
}

if BINPATH=$(find_release_binary); then
    install_release "$BINPATH"
else
    install_from_source
fi

say ""
say "running craftsman setup (installs the six skills, agent-aware)…"
"$CRAFTSMAN" setup
say ""
"$CRAFTSMAN" --version
say "done. next: craftsman init --name <project> --stack <stack> in a git repo."
