#!/bin/bash

git submodule update --init --recursive tools/emsdk
git submodule update --init --depth=1 tools/dawn
