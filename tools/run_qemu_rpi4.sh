#!/bin/bash
set -e

. tools/environment

RPI4_DIR=${RPI4_DIR:-$BASE_DIR/rpi4}
RPI4_IMAGE=${RPI4_IMAGE:-$RPI4_DIR/rpi4.img}
RPI4_MOUNT_DIR=${RPI4_MOUNT_DIR:-$BIN_DIR/rpi4-mnt}
RPI4_PARTITION_START_SECTOR=${RPI4_PARTITION_START_SECTOR:-2048}
LOOP_OFFSET=$((RPI4_PARTITION_START_SECTOR * 512))

cleanup() {
    if mountpoint -q "$RPI4_MOUNT_DIR" 2>/dev/null; then
        sudo umount "$RPI4_MOUNT_DIR"
    fi
}

trap cleanup EXIT

if [ "$#" -gt 0 ]; then
    if [ ! -f "$1" ]; then
        echo "Error: missing binary $1"
        exit 1
    fi
    if [ ! -f "$RPI4_IMAGE" ]; then
        echo "Error: missing $RPI4_IMAGE"
        echo "Create it first with rpi4/create_image.sh."
        exit 1
    fi

    cp "$1" "$RPI4_DIR/min1.elf"
    mkdir -p "$RPI4_MOUNT_DIR"
    sudo mount -o "loop,offset=$LOOP_OFFSET" "$RPI4_IMAGE" "$RPI4_MOUNT_DIR"
    sudo cp "$RPI4_DIR/min1.elf" "$RPI4_MOUNT_DIR/min1.elf"
    sync
    sudo umount "$RPI4_MOUNT_DIR"
fi

tools/launch_qemu_rpi4.sh
