#!/bin/bash
set -euo pipefail

. tools/environment

VERSION="2025.05"
ARCHIVE="buildroot-${VERSION}.tar.xz"
URL="https://buildroot.org/downloads/${ARCHIVE}"

# ディレクトリ設定
# 以前と異なる場所でビルドするため、独自のパスを指定
WSL_WORK_ROOT="${HOME}/.cache/min1visor-build-initrd"
WSL_BUILD_DIR="${WSL_WORK_ROOT}/buildroot"
SRC_DIR="${WSL_BUILD_DIR}/buildroot-${VERSION}"

# 既存のディレクトリをきれいに削除して再作成
rm -rf "$WSL_BUILD_DIR"
mkdir -p "$WSL_BUILD_DIR"
mkdir -p "$DISK_IMG_DIR"

pushd "$WSL_BUILD_DIR" >/dev/null

echo "Downloading ${ARCHIVE} ..."
curl -fL --retry 3 --retry-delay 2 -o "$ARCHIVE" "$URL"

echo "Extracting ${ARCHIVE} ..."
tar -xJf "$ARCHIVE"

pushd "$SRC_DIR" >/dev/null

# カーネル設定の修正 (CONFIG_BLK_DEV_INITRD=y の追加)
echo "Adding CONFIG_BLK_DEV_INITRD=y to kernel config..."
# 設定ファイルの末尾に追記する
# Initramfsの使用を可能にする
echo "CONFIG_BLK_DEV_INITRD=y" >> board/qemu/aarch64-virt/linux.config
# devtmpfs（デバイス自動作成機能）とその自動マウントを有効にする
echo "CONFIG_DEVTMPFS=y" >> board/qemu/aarch64-virt/linux.config
echo "CONFIG_DEVTMPFS_MOUNT=y" >> board/qemu/aarch64-virt/linux.config

export FORCE_UNSAFE_CONFIGURE=1

# PATHにWindows由来の空白パスなどが混ざる場合の対策
if echo "$PATH" | grep -q ' '; then
    export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
fi

# デフォルト設定の適用
make qemu_aarch64_virt_defconfig

# Buildroot設定の修正 (CPIOイメージ出力の有効化)
echo "Modifying Buildroot config for initramfs..."
# rootfs.cpio の生成を有効化
sed -i 's/^# BR2_TARGET_ROOTFS_CPIO is not set$/BR2_TARGET_ROOTFS_CPIO=y/' .config

# host-qemuの無効化（元のスクリプトを踏襲）
sed -i \
    -e 's/^BR2_PACKAGE_HOST_QEMU=y$/BR2_PACKAGE_HOST_QEMU=n/' \
    .config

# 設定の整合性を調整
make olddefconfig

# ビルド実行
echo "Starting build..."
make -j"$(nproc)"

# 成果物のコピー
cp output/images/Image "$DISK_IMG_DIR/Image"

# 今回新しく生成される cpio アーカイブをコピー
if [ -f output/images/rootfs.cpio ]; then
    cp output/images/rootfs.cpio "$DISK_IMG_DIR/rootfs.cpio"
    echo "Successfully copied rootfs.cpio to $DISK_IMG_DIR"
else
    echo "Error: rootfs.cpio was not generated!" >&2
    exit 1
fi

# ext2もコピー（不要なら削除）
cp output/images/rootfs.ext2 "$DISK_IMG_DIR/DISK0"

popd >/dev/null
popd >/dev/null

rm -rf "$WSL_BUILD_DIR"

echo "Build complete."
echo "Artifacts available in: $DISK_IMG_DIR"