#!/bin/bash

# Run this, then press ctrl+C to stop profiling
CARGO_PROFILE_RELEASE_DEBUG=true sudo cargo flamegraph --root --bin heisenbase -- generate KQvK
