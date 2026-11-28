#!/bin/sh
# Checksum-verifying installer for catchledger prebuilt release archives.
set -eu

REPO="david-engelmann/catchledger"
BIN="catchledger"
DEFAULT_INSTALL_DIR="${HOME:-}/.local/bin"
INSTALL_DIR="${CATCHLEDGER_INSTALL_DIR:-$DEFAULT_INSTALL_DIR}"
VERSION="${CATCHLEDGER_VERSION:-latest}"

if [ -z "${CATCHLEDGER_INSTALL_DIR:-}" ] && [ -z "${HOME:-}" ]; then
    echo "catchledger install error, HOME is not set so pass CATCHLEDGER_INSTALL_DIR explicitly" >&2
    exit 1
fi

need_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "catchledger install error, required command '$1' not found" >&2
        exit 1
    fi
}

need_cmd curl
need_cmd tar
need_cmd uname
need_cmd mktemp
need_cmd awk
need_cmd sed
need_cmd head
need_cmd tr

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux) OS_PART="unknown-linux-gnu" ;;
    Darwin) OS_PART="apple-darwin" ;;
    *)
        echo "catchledger install error, unsupported operating system '$OS' (Linux and macOS prebuilt archives are supported)" >&2
        exit 1
        ;;
esac

case "$ARCH" in
    x86_64|amd64) ARCH_PART="x86_64" ;;
    aarch64|arm64) ARCH_PART="aarch64" ;;
    *)
        echo "catchledger install error, unsupported architecture '$ARCH'" >&2
        exit 1
        ;;
esac

TARGET="${ARCH_PART}-${OS_PART}"

if [ "$VERSION" = "latest" ]; then
    if [ -n "${CATCHLEDGER_RELEASES_API:-}" ]; then
        TAG="$(curl -fsSL "$CATCHLEDGER_RELEASES_API" | tr ',' '\n' | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
    else
        FINAL_URL="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/${REPO}/releases/latest" || true)"
        TAG="$(printf '%s\n' "$FINAL_URL" | sed -n 's#.*/releases/tag/\([^/?#]*\).*#\1#p' | head -n 1)"
        if [ -z "$TAG" ]; then
            API_URL="https://api.github.com/repos/${REPO}/releases/latest"
            TAG="$(curl -fsSL "$API_URL" | tr ',' '\n' | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
        fi
    fi
    if [ -z "$TAG" ]; then
        echo "catchledger install error, could not resolve latest release tag for ${REPO}" >&2
        exit 1
    fi
else
    case "$VERSION" in
        v*) TAG="$VERSION" ;;
        *) TAG="v$VERSION" ;;
    esac
fi

case "$TAG" in
    ''|*[!A-Za-z0-9._+-]*)
        echo "catchledger install error, invalid release tag '$TAG'" >&2
        exit 1
        ;;
esac

ARCHIVE="${BIN}-${TAG}-${TARGET}.tar.gz"
BASE_URL="${CATCHLEDGER_RELEASE_BASE_URL:-https://github.com/${REPO}/releases/download/${TAG}}"

TMP_DIR="$(mktemp -d)"
STAGE_BIN=""
cleanup() {
    rm -rf "$TMP_DIR"
    if [ -n "$STAGE_BIN" ] && [ -f "$STAGE_BIN" ]; then
        rm -f "$STAGE_BIN"
    fi
}
trap cleanup EXIT INT TERM

if ! curl -fsSL "${BASE_URL}/${ARCHIVE}" -o "${TMP_DIR}/${ARCHIVE}"; then
    echo "catchledger install error, prebuilt archive ${ARCHIVE} is not available at ${BASE_URL} (build from source with cargo install --git https://github.com/${REPO} --locked)" >&2
    exit 1
fi

if ! curl -fsSL "${BASE_URL}/${ARCHIVE}.sha256" -o "${TMP_DIR}/${ARCHIVE}.sha256"; then
    echo "catchledger install error, checksum file ${ARCHIVE}.sha256 is not available at ${BASE_URL}" >&2
    exit 1
fi

EXPECTED="$(awk '{print $1}' "${TMP_DIR}/${ARCHIVE}.sha256")"
if [ -z "$EXPECTED" ]; then
    echo "catchledger install error, checksum file ${ARCHIVE}.sha256 is empty" >&2
    exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL="$(sha256sum "${TMP_DIR}/${ARCHIVE}" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
    ACTUAL="$(shasum -a 256 "${TMP_DIR}/${ARCHIVE}" | awk '{print $1}')"
else
    echo "catchledger install error, neither sha256sum nor shasum is available to verify ${ARCHIVE}" >&2
    exit 1
fi

if [ "$EXPECTED" != "$ACTUAL" ]; then
    echo "catchledger install error, SHA-256 mismatch for ${ARCHIVE} (expected ${EXPECTED}, got ${ACTUAL})" >&2
    exit 1
fi

tar -xzf "${TMP_DIR}/${ARCHIVE}" -C "$TMP_DIR"
mkdir -p "$INSTALL_DIR"
if [ -d "${INSTALL_DIR}/${BIN}" ]; then
    echo "catchledger install error, destination ${INSTALL_DIR}/${BIN} exists and is a directory" >&2
    exit 1
fi
STAGE_BIN="${INSTALL_DIR}/.${BIN}.tmp.$$"
cp "${TMP_DIR}/${BIN}" "$STAGE_BIN"
chmod 0755 "$STAGE_BIN"
mv -f "$STAGE_BIN" "${INSTALL_DIR}/${BIN}"
STAGE_BIN=""

echo "installed ${BIN} (${TAG}, ${TARGET}) to ${INSTALL_DIR}/${BIN}"
