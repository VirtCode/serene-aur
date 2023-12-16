#!/bin/sh

# enter build directory
sudo chown -R build build
cd build || exit

# fix fakeroot problem (https://github.com/moby/moby/issues/45436)
ulimit -u 62811 && ulimit -n 1024

# update container
sudo pacman -Suy --noconfirm

# build
rm -rf ./serene-build/*
makepkg -sf --noconfirm
mv ./*.pkg.tar.* serene-build/