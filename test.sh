#!/usr/bin/env bash

# in order to run the tests we need to have a bulb at 192.168.1.91
# also, we have to use single threaded tests b/c the bulb API is UDP
cargo test -- --test-threads=1
