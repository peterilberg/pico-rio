#!/usr/bin/bash

pushd messages
cargo build
popd

pushd pico
cargo run --release
popd

pushd tools
cargo build
popd
