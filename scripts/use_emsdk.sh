#!/bin/bash

# Make the "latest" SDK "active" for the current user. (writes .emscripten file)
./tools/emsdk/emsdk activate latest

# Activate PATH and other environment variables in the current terminal
source ./tools/emsdk/emsdk_env.sh
