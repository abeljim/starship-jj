#!/bin/sh
set -eu

REPO="abeljim/starship-jj"
BINARY="starship-jj"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

detect_target() {
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux)
            case "$arch" in
                x86_64)  echo "x86_64-unknown-linux-gnu" ;;
                aarch64) echo "aarch64-unknown-linux-gnu" ;;
                *) echo "Unsupported architecture: $arch" >&2; exit 1 ;;
            esac
            ;;
        Darwin)
            case "$arch" in
                x86_64)  echo "x86_64-apple-darwin" ;;
                arm64)   echo "aarch64-apple-darwin" ;;
                *) echo "Unsupported architecture: $arch" >&2; exit 1 ;;
            esac
            ;;
        *)
            echo "Unsupported OS: $os" >&2
            echo "For Windows, download the binary from https://github.com/$REPO/releases" >&2
            exit 1
            ;;
    esac
}

main() {
    target="$(detect_target)"
    echo "Detected target: $target"

    if [ -z "${VERSION:-}" ]; then
        VERSION="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | cut -d '"' -f 4)"
    fi
    echo "Installing $BINARY $VERSION"

    archive="$BINARY-$VERSION-$target.tar.gz"
    url="https://github.com/$REPO/releases/download/$VERSION/$archive"

    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' EXIT

    echo "Downloading $url"
    curl -fsSL "$url" -o "$tmpdir/$archive"
    tar xzf "$tmpdir/$archive" -C "$tmpdir"

    mkdir -p "$INSTALL_DIR"
    mv "$tmpdir/$BINARY" "$INSTALL_DIR/$BINARY"
    chmod +x "$INSTALL_DIR/$BINARY"
    echo "$BINARY installed to $INSTALL_DIR/$BINARY"
}

main
