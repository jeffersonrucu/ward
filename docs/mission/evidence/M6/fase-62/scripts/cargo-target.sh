#!/bin/bash -p
# fase62: runs one integration test target of the wardd crate. Usage: cargo-target.sh <root> <target>
export PATH=/usr/bin:/bin
cd "$1" || exit 2
exec <home>/.cargo/bin/cargo test --locked -p wardd --all-features --test "$2"
