#!/bin/bash
set -euo pipefail

. tools/environment

VERSION="2025.05"
ARCHIVE="buildroot-${VERSION}.tar.xz"
URL="https://buildroot.org/downloads/${ARCHIVE}"

# Buildroot本体はWSLのLinux領域でビルドする
WSL_WORK_ROOT="${HOME}/.cache/min1visor-build"
WSL_BUILD_DIR="${WSL_WORK_ROOT}/buildroot"
SRC_DIR="${WSL_BUILD_DIR}/buildroot-${VERSION}"

rm -rf "$WSL_BUILD_DIR"
mkdir -p "$WSL_BUILD_DIR"
mkdir -p "$DISK_IMG_DIR"

pushd "$WSL_BUILD_DIR" >/dev/null

echo "Downloading ${ARCHIVE} ..."
curl -fL --retry 3 --retry-delay 2 -o "$ARCHIVE" "$URL"

echo "Extracting ${ARCHIVE} ..."
tar -xJf "$ARCHIVE"

pushd "$SRC_DIR" >/dev/null

export FORCE_UNSAFE_CONFIGURE=1

# PATHにWindows由来の空白パスなどが混ざる場合の対策
if echo "$PATH" | grep -q ' '; then
    export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
fi

make qemu_aarch64_virt_defconfig

# host-qemu不要なら無効化
sed -i \
    -e 's/^BR2_PACKAGE_HOST_QEMU=y$/BR2_PACKAGE_HOST_QEMU=n/' \
    .config

# 念のため反映
make olddefconfig

# glibc問題切り分けも兼ねて負荷を抑えたいなら -j1 に変更可
make -j"$(nproc)"

cp output/images/Image "$DISK_IMG_DIR/Image"
cp output/images/rootfs.ext2 "$DISK_IMG_DIR/DISK0"

popd >/dev/null
popd >/dev/null

rm -rf "$WSL_BUILD_DIR"

echo "Buildroot artifacts copied to:"
echo "  $DISK_IMG_DIR/Image"
echo "  $DISK_IMG_DIR/DISK0"