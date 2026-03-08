#!/bin/bash
cargo espflash flash --monitor --release --bin omniserver --target xtensa-esp32-espidf --baud 115200
