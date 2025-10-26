#!/bin/bash

cargo build --release
echo 'built - moving to ~/.local/bin'
cp target/release/agenda-core ~/.local/bin/
