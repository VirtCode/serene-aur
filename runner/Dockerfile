FROM archlinux:base-devel

# create build user
RUN useradd -m build && echo "build ALL=(ALL) NOPASSWD: ALL" > /etc/sudoers.d/build

# update pacman and install packages
RUN pacman -Suy --noconfirm
RUN pacman -Sy --noconfirm git

# create build folder and script
ADD run.sh /app/run.sh
RUN mkdir -p /app/build/serene-build

# switch to builder user
USER build
WORKDIR /app

# run
CMD sh run.sh