#!/bin/sh
# QEMU launcher script for AURA OS UEFI target
qemu-system-x86_64 \
    -bios /usr/share/ovmf/OVMF.fd \
    -drive format=raw,file=fat:rw:target/x86_64-unknown-uefi/debug \
    -net none \
    -nographic
