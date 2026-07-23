#!/bin/bash
set -e

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
BASE_DIR=$(cd "$SCRIPT_DIR/.." && pwd)
BIN_DIR=$BASE_DIR/bin

RPI4_DIR=${RPI4_DIR:-$SCRIPT_DIR}
RPI4_IMAGE=${RPI4_IMAGE:-${IMAGE:-$RPI4_DIR/rpi4.img}}
RPI4_IMAGE_SIZE_MB=${RPI4_IMAGE_SIZE_MB:-${IMAGE_SIZE_MB:-256}}
RPI4_MOUNT_DIR=${RPI4_MOUNT_DIR:-${MOUNT_POINT:-$BIN_DIR/rpi4-mnt}}
RPI4_PARTITION_START_SECTOR=${RPI4_PARTITION_START_SECTOR:-${PARTITION_START_SECTOR:-2048}}
RPI4_KERNEL=${RPI4_KERNEL:-$BASE_DIR/target/aarch64-unknown-none-softfloat/release/min1visor}
RPI4_BOOTPART=${RPI4_BOOTPART:-1:1}
LOOP_DEV=""

FILES=(
    "start4.elf"
    "fixup4.dat"
    "u-boot.bin"
    "config.txt"
    "bcm2711-rpi-4-b.dtb"
    "boot.scr"
    "min1.elf"
    "Image"
    "guest.dtb"
    "rootfs.cpio.gz"
)

cleanup() {
    if mountpoint -q "$RPI4_MOUNT_DIR" 2>/dev/null; then
        echo "[cleanup] Unmounting $RPI4_MOUNT_DIR ..."
        sudo umount "$RPI4_MOUNT_DIR"
    fi
    if [ -n "$LOOP_DEV" ]; then
        echo "[cleanup] Detaching $LOOP_DEV ..."
        sudo losetup -d "$LOOP_DEV"
    fi
}

trap cleanup EXIT

if [ ! -f "$RPI4_KERNEL" ]; then
    echo "Error: missing kernel binary $RPI4_KERNEL"
    echo "Build it first with: cargo build --release"
    exit 1
fi

if [ -f "$RPI4_IMAGE" ]; then
    rm -f "$RPI4_IMAGE"
fi

echo "[1/6] Creating $RPI4_IMAGE (${RPI4_IMAGE_SIZE_MB} MiB) ..."
dd if=/dev/zero of="$RPI4_IMAGE" bs=1M count="$RPI4_IMAGE_SIZE_MB"

echo "[2/6] Creating FAT32 partition ..."
sudo sfdisk "$RPI4_IMAGE" << EOF
$RPI4_PARTITION_START_SECTOR,,b,*
EOF

echo "[3/6] Formatting partition ..."
sudo mkfs.vfat -F 32 --offset="$RPI4_PARTITION_START_SECTOR" "$RPI4_IMAGE"

echo "[4/6] Generating guest DTB and boot.scr ..."
ENTRY_POINT=$(readelf -h "$RPI4_KERNEL" \
    | grep "Entry point address" \
    | awk '{print $4}')
echo "entry point: $ENTRY_POINT"

dtc -I dts -O dtb -o "$RPI4_DIR/guest.dtb" "$RPI4_DIR/guest.dts"
sed \
    -e "s/@HYPERVISOR_ENTRY@/$ENTRY_POINT/" \
    -e "s/@BOOTPART@/$RPI4_BOOTPART/" \
    "$RPI4_DIR/boot.txt.template" > "$RPI4_DIR/boot.txt"

mkimage -A arm64 -T script -C none -n "RPi4 Boot Script" \
    -d "$RPI4_DIR/boot.txt" "$RPI4_DIR/boot.scr"


echo "[5/6] Mounting image ..."
LOOP_DEV=$(sudo losetup -Pf --show "$RPI4_IMAGE")
sudo mkdir -p "$RPI4_MOUNT_DIR"
sudo mount "${LOOP_DEV}p1" "$RPI4_MOUNT_DIR"

echo "[6/6] Copying files ..."
cp "$RPI4_KERNEL" "$RPI4_DIR/min1.elf"

COPIED=0
for f in "${FILES[@]}"; do
    if [ -f "$RPI4_DIR/$f" ]; then
        sudo cp "$RPI4_DIR/$f" "$RPI4_MOUNT_DIR/"
        echo "      copied: $f"
        COPIED=$((COPIED + 1))
    else
        echo "      skipped: $f"
    fi
done

sync
sudo umount "$RPI4_MOUNT_DIR"
sudo losetup -d "$LOOP_DEV"
LOOP_DEV=""

echo ""
echo "Done. $RPI4_IMAGE created ($COPIED file(s) copied)."
