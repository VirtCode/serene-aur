#!/bin/sh

. ./prepare.sh

# update container
message "updating system"
sudo pacman -Suy --noconfirm

# prepare
message "running per-package preparation commands"
. ./serene-prepare.sh

# build
message "starting package build"
# read additional flags
FLAGS=$(cat makepkg-flags)
echo "running with custom flags: $FLAGS"

# run makepkg
makepkg --syncdeps --force --noconfirm $FLAGS

# also add built version, primarily for devel packages
message "collecting package information"
makepkg --printsrcinfo > ../target/.SRCINFO

message "build script finished"
