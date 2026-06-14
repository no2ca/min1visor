#!/bin/bash
set -e

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
BASE_DIR=$(cd "$SCRIPT_DIR/.." && pwd)
BIN_DIR=$BASE_DIR/bin
QEMU_DIR=$BIN_DIR/qemu

RPI4_DIR=${RPI4_DIR:-$BASE_DIR/rpi4}
RPI4_IMAGE=${RPI4_IMAGE:-$RPI4_DIR/rpi4.img}
RPI4_UBOOT=${RPI4_UBOOT:-$RPI4_DIR/u-boot.bin}
RPI4_DTB=${RPI4_DTB:-$RPI4_DIR/bcm2711-rpi-4-b.dtb}
RPI4_MACHINE=${RPI4_MACHINE:-raspi4b}
RPI4_MEMORY=${RPI4_MEMORY:-2G}
RPI4_SMP=${RPI4_SMP:-4}
RPI4_SD_BACKEND=${RPI4_SD_BACKEND:-sd}

QEMU=${QEMU:-}
if [ -z "$QEMU" ]; then
    QEMU=$(command -v "$QEMU_DIR/bin/qemu-system-aarch64" 2>/dev/null || true)
fi
if [ -z "$QEMU" ]; then
    QEMU=$(command -v qemu-system-aarch64 2>/dev/null || true)
fi

if [ -z "$QEMU" ]; then
    echo "Error: qemu-system-aarch64 is not found."
    exit 1
fi

if ! "$QEMU" -machine help | grep -q "^$RPI4_MACHINE[[:space:]]"; then
    echo "Error: $QEMU does not support -M $RPI4_MACHINE."
    echo "Build a QEMU version with Raspberry Pi 4 support, or set RPI4_MACHINE to a supported machine."
    exit 1
fi

for f in "$RPI4_IMAGE" "$RPI4_UBOOT" "$RPI4_DTB"; do
    if [ ! -f "$f" ]; then
        echo "Error: missing $f"
        echo "Create the image first with: $BASE_DIR/rpi4/create_image.sh"
        exit 1
    fi
done

case "$RPI4_SD_BACKEND" in
    sd)
        SD_ARGS=(-sd "$RPI4_IMAGE")
        ;;
    drive)
        SD_ARGS=(-drive "file=$RPI4_IMAGE,format=raw,if=sd")
        ;;
    *)
        echo "Error: unknown RPI4_SD_BACKEND=$RPI4_SD_BACKEND"
        echo "Use RPI4_SD_BACKEND=sd or RPI4_SD_BACKEND=drive."
        exit 1
        ;;
esac

exec "$QEMU" \
    -M "$RPI4_MACHINE" \
    -smp "$RPI4_SMP" \
    -m "$RPI4_MEMORY" \
    -kernel "$RPI4_UBOOT" \
    -dtb "$RPI4_DTB" \
    "${SD_ARGS[@]}" \
    -serial mon:stdio \
    -display none \
    -no-reboot
