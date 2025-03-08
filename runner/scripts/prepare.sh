#!/bin/sh

# some aliases / functions
message() {
  printf "\n:: SERENE :: %s\n\n" "$1"
}

message "preparing container"

# manage target folder
sudo chown -R build target
rm -rf target/*

# enter build directory
sudo chown -R build build
cd build || exit

# exit on errors
set -e

# fix fakeroot problem (https://github.com/moby/moby/issues/45436)
ulimit -u 62811 && ulimit -n 1024
