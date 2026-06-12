#!/bin/bash
set -e

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)

IMAGE="${IMAGE:-$SCRIPT_DIR/rpi4.img}"
IMAGE_SIZE_MB="${IMAGE_SIZE_MB:-256}"
MOUNT_POINT="${MOUNT_POINT:-/mnt/rpi4boot}"
PARTITION_START_SECTOR="${PARTITION_START_SECTOR:-2048}"
LOOP_DEV=""

FILES=(
    "start4.elf"
    "fixup4.dat"
    "u-boot.bin"
    "config.txt"
    "bcm2711-rpi-4-b.dtb"
    "min1.elf"
)

cleanup() {
    if mountpoint -q "$MOUNT_POINT" 2>/dev/null; then
        echo "[cleanup] Unmounting $MOUNT_POINT ..."
        sudo umount "$MOUNT_POINT"
    fi
    if [ -n "$LOOP_DEV" ]; then
        echo "[cleanup] Detaching $LOOP_DEV ..."
        sudo losetup -d "$LOOP_DEV"
    fi
}

trap cleanup EXIT

if [ -f "$IMAGE" ]; then
    if [ "${FORCE:-0}" != "1" ]; then
        echo "Error: $IMAGE already exists."
        echo "Set FORCE=1 to recreate it."
        exit 1
    fi

    rm -f "$IMAGE"
fi

echo "[1/5] Creating $IMAGE (${IMAGE_SIZE_MB} MiB) ..."
dd if=/dev/zero of="$IMAGE" bs=1M count="$IMAGE_SIZE_MB"

echo "[2/5] Creating FAT32 partition ..."
sudo sfdisk "$IMAGE" << EOF
$PARTITION_START_SECTOR,,b,*
EOF

echo "[3/5] Formatting partition ..."
sudo mkfs.vfat -F 32 --offset="$PARTITION_START_SECTOR" "$IMAGE"

echo "[4/5] Mounting image ..."
LOOP_DEV=$(sudo losetup -Pf --show "$IMAGE")
sudo mkdir -p "$MOUNT_POINT"
sudo mount "${LOOP_DEV}p1" "$MOUNT_POINT"

echo "[5/5] Copying files ..."
cp ../target/aarch64-unknown-none-softfloat/release/min1visor ./min1.elf

COPIED=0
for f in "${FILES[@]}"; do
    if [ -f "$SCRIPT_DIR/$f" ]; then
        sudo cp "$SCRIPT_DIR/$f" "$MOUNT_POINT/"
        echo "      copied: $f"
        COPIED=$((COPIED + 1))
    else
        echo "      skipped: $f"
    fi
done

sync
sudo umount "$MOUNT_POINT"
sudo losetup -d "$LOOP_DEV"
LOOP_DEV=""

echo ""
echo "Done. $IMAGE created ($COPIED file(s) copied)."
