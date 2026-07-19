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

U-Boot のプロンプトから手で起動する場合は、Linux Image、ゲスト用 DTB、ハイパーバイザの順に読み込みます。
ホスト用の `bcm2711-rpi-4-b.dtb` はハイパーバイザの初期化に使い、ゲストには `0x0900_0000` の PL011 を記述した `guest.dtb` を渡します。

```sh
setenv linux_image_addr 0x10000000
setenv guest_dtb_addr 0x1f000000
fatload mmc 1:1 ${linux_image_addr} Image
fatload mmc 1:1 ${guest_dtb_addr} guest.dtb
fatload mmc 1:1 ${kernel_addr_r} min1.elf
go <min1visor-entry> 0x${fdt_addr} ${guest_dtb_addr} ${kernel_addr_r} ${linux_image_addr}
```

`<min1visor-entry>` には `readelf -h min1.elf` が表示するエントリーポイントを指定します。
Linux とゲスト DTB は U-Boot が認識する物理 RAM 内へロードし、ゲストからは Stage 2 変換を通してそれぞれ `0x4000_0000` と `0x4f00_0000` に見えます。

`-sd` で動かない QEMU では、元の `-drive if=sd` 指定に戻して試せます。

```sh
RPI4_SD_BACKEND=drive tools/launch_qemu_rpi4.sh
```
