#!/bin/sh

. ./prepare.sh

message "generating .SRCINFO"
makepkg --printsrcinfo > ../target/.SRCINFO

message "srcinfo script finished"
