#!/bin/bash

. tools/environment

VERSION="10.0.0"

rm -rf $QEMU_DIR
mkdir -p $QEMU_DIR

sudo env DEBIAN_FRONTEND=noninteractive apt install -y libssl-dev python3-venv ninja-build libglib2.0-dev libcapstone-dev
curl -fL https://download.qemu.org/qemu-$VERSION.tar.xz -o qemu-$VERSION.tar.xz
tar xvJf qemu-$VERSION.tar.xz
rm -f qemu-$VERSION.tar.xz

pushd qemu-$VERSION
./configure --target-list=aarch64-softmmu --prefix=$QEMU_DIR
make -j$(nproc)
make install
popd

rm -rf qemu-$VERSION
