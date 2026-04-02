#!/bin/bash
export RUSTFLAGS="-C link-arg=-Wl,-rpath,/opt/homebrew/lib"
cargo build
