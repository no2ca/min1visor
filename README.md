# min1visor

[作って理解する仮想化技術](https://gihyo.jp/book/2025/978-4-297-15012-9)を進めています

## ビルド対象

デフォルトは Raspberry Pi 4 向けです。

```sh
cargo build --release
```

QEMU の `virt` マシンで動作確認する場合は `qemu-virt` feature を指定します。

```sh
cargo run --no-default-features --features qemu-virt
```

QEMU の Raspberry Pi 4 マシンで動作確認する場合は、RPi4 向けのバイナリを SD イメージへ反映してから起動します。

```sh
cargo run --config 'target.aarch64-unknown-none-softfloat.runner="tools/run_qemu_rpi4.sh"'
```

手元の SD イメージをそのまま起動する場合は次のスクリプトを使います。

```sh
tools/launch_qemu_rpi4.sh
```

U-Boot で SD カードが見えない場合は、まず次を確認します。QEMU の Raspberry Pi 4 では SD カードが `mmc 1` として見えることがあります。

```sh
mmc list
mmc dev 1
fatls mmc 1:1
```

U-Boot のプロンプトから手で起動する場合は、`mmc 1:1` から `min1.elf` を読みます。

```sh
if fatload mmc 1:1 $kernel_addr_r min1.elf; then
    bootelf $kernel_addr_r $fdt_addr $kernel_addr_r
else
    echo "Unable to read min1.elf"
fi
```

`-sd` で動かない QEMU では、元の `-drive if=sd` 指定に戻して試せます。

```sh
RPI4_SD_BACKEND=drive tools/launch_qemu_rpi4.sh
```
