#!/bin/sh

. ./prepare.sh

# update container
message "updating system"
sudo pacman -Suy --noconfirm

# prepare
message "running per-package preparation commands"
. ./serene-prepare.sh

# dependency sync
message "synchronizing dependencies"
makepkg --syncdeps --noprepare --nobuild --noconfirm

# collect stats before build
(. ../stats.sh > ../target/.stats-before.json)

# build
message "starting package build"
# read additional flags
FLAGS=$(cat makepkg-flags)
echo "running with custom flags: $FLAGS"

# run makepkg
makepkg --force --noconfirm $FLAGS

# collect stats after build
(. ../stats.sh > ../target/.stats-after.json)

# also add built version, primarily for devel packages
message "collecting package information"
makepkg --printsrcinfo > ../target/.SRCINFO

message "cleaning up to save space"

# we have no use for the pacman cache
# - use yes because --noconfirm doesn't work for this
# - for some reason it can also return a non-zero exit code after some time, so
#   cause of set -e the exit code will be non-zero even though the package built
yes | sudo pacman -Scc || true

message "build script finished"
