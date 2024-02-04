#!/bin/bash

cargo skyline build --release && cp ./target/aarch64-skyline-switch/release/libsmashline_plugin.nro $YUZU_MODS/libsmashline_plugin.nro
