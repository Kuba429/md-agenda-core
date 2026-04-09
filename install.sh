#!/bin/bash

cargo build --release
echo 'built - stripping and moving to ~/.local/bin'
strip -s target/release/agenda-core -o target/release/agenda-core-stripped
mv target/release/agenda-core-stripped ~/.local/bin/agenda-core
