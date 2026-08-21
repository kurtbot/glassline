#!/bin/sh
# glassline installer — POSIX sh, no bash-only features.
#
# Detects OS/arch, downloads the matching release archive from GitHub,
# verifies its SHA256 against the release's SHA256SUMS.txt, extracts the
# `glassline` binary to $GLASSLINE_INSTALL_DIR (default: ~/.local/bin).
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/kurtbot/glassline/main/packaging/install.sh | sh
#   curl -fsSL https://raw.githubusercontent.com/kurtbot/glassline/main/packaging/install.sh | sh -s -- --version v0.5.0
#   curl -fsSL https://raw.githubusercontent.com/kurtbot/glassline/main/packaging/install.sh | sh -s -- --dir /usr/local/bin
#
# Env overrides:
#   GLASSLINE_INSTALL_DIR — where to drop the binary. Default: $HOME/.local/bin.

set -eu

REPO="kurtbot/glassline"
VERSION="latest"
INSTALL_DIR="${GLASSLINE_INSTALL_DIR:-$HOME/.local/bin}"
UPDATE_PATH="1"

# ---------- argv ----------

while [ $# -gt 0 ]; do
  case "$1" in
    --version)
      VERSION="$2"
      shift 2
      ;;
    --dir)
      INSTALL_DIR="$2"
      shift 2
      ;;
    --no-path)
      UPDATE_PATH="0"
      shift 1
      ;;
    -h|--help)
      cat <<'EOF'
glassline installer

Usage:
  install.sh [--version vX.Y.Z] [--dir /path] [--no-path]

Options:
  --version   Tag to install (default: latest).
  --dir       Install directory (default: $HOME/.local/bin).
  --no-path   Skip PATH modification; print instructions instead.
EOF
      exit 0
      ;;
    *)
      echo "unknown flag: $1" >&2
      exit 2
      ;;
  esac
done

# ---------- shell rc updater ----------

# Append `export PATH="$PATH:<dir>"` (or fish equivalent) to the user's
# shell rc file, bracketed by marker comments so re-running the
# installer is idempotent and users can remove the block by hand.
#
# Detection: prefer $SHELL basename; fall back to ~/.profile for
# unknown shells so PATH still gets updated for POSIX-shell logins.
update_shell_rc() {
  target_dir="$1"
  shell_bin="${SHELL##*/}"
  rc=""
  block_type="posix"
  case "$shell_bin" in
    bash) rc="$HOME/.bashrc" ;;
    zsh)  rc="$HOME/.zshrc" ;;
    fish)
      rc="$HOME/.config/fish/config.fish"
      block_type="fish"
      mkdir -p "$(dirname "$rc")"
      ;;
    *) rc="$HOME/.profile" ;;
  esac

  if [ -f "$rc" ] && grep -q '# >>> glassline install >>>' "$rc" 2>/dev/null; then
    echo "$rc already has a glassline PATH block — skipping."
    echo "  restart your shell (or 'source $rc') to pick it up."
    return 0
  fi

  {
    printf '\n# >>> glassline install >>>\n'
    if [ "$block_type" = "fish" ]; then
      printf 'if not contains %s $PATH\n    set -gx PATH $PATH %s\nend\n' "$target_dir" "$target_dir"
    else
      printf 'case ":$PATH:" in\n  *":%s:"*) ;;\n  *) export PATH="$PATH:%s" ;;\nesac\n' "$target_dir" "$target_dir"
    fi
    printf '# <<< glassline install <<<\n'
  } >> "$rc"

  echo ""
  echo "added $target_dir to PATH via $rc"
  echo "  restart your shell (or 'source $rc') to pick it up in this session."
  echo "  pass --no-path to future installer runs to skip this."
}

# ---------- os/arch detection ----------

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Linux)
    case "$arch" in
      x86_64|amd64) target="x86_64-unknown-linux-gnu" ;;
      aarch64|arm64) target="aarch64-unknown-linux-gnu" ;;
      *) echo "unsupported linux arch: $arch" >&2; exit 1 ;;
    esac
    archive_ext="tar.gz"
    ;;
  Darwin)
    case "$arch" in
      x86_64) target="x86_64-apple-darwin" ;;
      arm64) target="aarch64-apple-darwin" ;;
      *) echo "unsupported darwin arch: $arch" >&2; exit 1 ;;
    esac
    archive_ext="tar.gz"
    ;;
  *)
    echo "unsupported OS: $os. For Windows use install.ps1." >&2
    exit 1
    ;;
esac

# ---------- resolve version ----------

if [ "$VERSION" = "latest" ]; then
  echo "resolving latest release..."
  VERSION="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
    | grep -o '"tag_name":[[:space:]]*"[^"]*"' \
    | head -1 \
    | sed -E 's/.*"([^"]+)"$/\1/')"
  if [ -z "$VERSION" ]; then
    echo "could not resolve latest release tag from GitHub API" >&2
    exit 1
  fi
fi

archive="glassline-${target}.${archive_ext}"
base="https://github.com/${REPO}/releases/download/${VERSION}"
archive_url="${base}/${archive}"
sums_url="${base}/SHA256SUMS.txt"

echo "target:  $target"
echo "version: $VERSION"
echo "archive: $archive_url"
echo "dir:     $INSTALL_DIR"

# ---------- download ----------

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

echo "downloading archive..."
curl -fsSL -o "$tmp/$archive" "$archive_url"

echo "downloading SHA256SUMS.txt..."
if ! curl -fsSL -o "$tmp/SHA256SUMS.txt" "$sums_url"; then
  echo "warning: SHA256SUMS.txt not found on release. Skipping verification." >&2
  echo "         (releases prior to v0.5.1 don't ship the sums file yet.)" >&2
else
  expected="$(grep -E "  $archive$" "$tmp/SHA256SUMS.txt" | awk '{print $1}')"
  if [ -z "$expected" ]; then
    echo "no SHA256 line for $archive in SHA256SUMS.txt" >&2
    exit 1
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    actual="$(sha256sum "$tmp/$archive" | awk '{print $1}')"
  elif command -v shasum >/dev/null 2>&1; then
    actual="$(shasum -a 256 "$tmp/$archive" | awk '{print $1}')"
  else
    echo "no sha256 tool (sha256sum / shasum) found" >&2
    exit 1
  fi
  if [ "$expected" != "$actual" ]; then
    echo "SHA256 mismatch!" >&2
    echo "  expected: $expected" >&2
    echo "  actual:   $actual" >&2
    exit 1
  fi
  echo "sha256 OK"
fi

# ---------- extract + install ----------

echo "extracting..."
mkdir -p "$tmp/extract"
tar -xzf "$tmp/$archive" -C "$tmp/extract"

mkdir -p "$INSTALL_DIR"

# Install both the render binary and the editor. The editor became part
# of the archive in v0.6.2; older archives ship only `glassline`, so a
# missing sibling is a warning, not a fatal error.
installed_any=0
for name in glassline glassline-tui; do
  path="$tmp/extract/$name"
  if [ ! -f "$path" ]; then
    found="$(find "$tmp/extract" -name "$name" -type f | head -1)"
    if [ -z "$found" ]; then
      if [ "$name" = "glassline" ]; then
        echo "glassline binary not found in archive" >&2
        exit 1
      else
        echo "note: $name not in this archive (pre-v0.6.2). Interactive editor won't launch." >&2
        continue
      fi
    fi
    path="$found"
  fi
  install -m 0755 "$path" "$INSTALL_DIR/$name"
  echo "installed: $INSTALL_DIR/$name"
  installed_any=1
done

if [ "$installed_any" = "0" ]; then
  echo "no binaries installed" >&2
  exit 1
fi

case ":$PATH:" in
  *":$INSTALL_DIR:"*)
    echo "PATH already contains $INSTALL_DIR — you're set."
    ;;
  *)
    if [ "$UPDATE_PATH" = "1" ]; then
      update_shell_rc "$INSTALL_DIR"
    else
      echo ""
      echo "NOTE: $INSTALL_DIR is not on your PATH (--no-path given)."
      echo "  Add this line to your shell rc (.bashrc / .zshrc / .config/fish/config.fish):"
      echo "    export PATH=\"\$PATH:$INSTALL_DIR\""
    fi
    ;;
esac

echo ""
echo "next: run 'glassline install' to wire it into ~/.claude/settings.json"
