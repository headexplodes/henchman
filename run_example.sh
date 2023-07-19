#!/bin/sh

RUST_LOG=info cargo run -- -d example -l 127.0.0.1:8080
