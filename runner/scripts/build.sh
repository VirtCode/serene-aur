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
makepkg --syncdeps --nobuild --noconfirm

# collect stats before build
(. /app/stats.sh > /app/target/.stats-before.json)

# build
message "starting package build"
# read additional flags
FLAGS=$(cat makepkg-flags)
echo "running with custom flags: $FLAGS"

# run makepkg
makepkg --force --noconfirm $FLAGS

# collect stats after build
(. /app/stats.sh > /app/target/.stats-after.json)

# also add built version, primarily for devel packages
message "collecting package information"
makepkg --printsrcinfo > ../target/.SRCINFO

message "cleaning up to save space"
# we have no use for the pacman cache (and yes --noconfirm doesn't work here)
yes | sudo pacman -Scc

message "build script finished"
