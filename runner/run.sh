#!/bin/sh

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

# update container
printf "\n\n:: SERENE :: Updating system\n"
sudo pacman -Suy --noconfirm

# prepare
printf "\n\n:: SERENE :: Running preparation commands\n"
source ./serene-prepare.sh

# build
printf "\n\n:: SERENE :: Building package\n"
# read additional flags
FLAGS=$(cat makepkg-flags)
echo "running with custom flags: $FLAGS"
# run makepkg
makepkg --syncdeps --force --noconfirm $FLAGS

# also add built version, primarily for devel packages
printf "\n\n:: SERENE :: Collecting package information\n"
makepkg --printsrcinfo > ../target/.SRCINFO
# for backwards compatibility, I don't know how this could work without the . in VERSION
cat ../target/.SRCINFO | grep -oP 'pkgver = \K[^ ]+' > ../target/VERSION

echo "build script finished";
