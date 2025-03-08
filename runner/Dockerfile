FROM archlinux:base-devel

# create build user
RUN useradd -m build && echo "build ALL=(ALL) NOPASSWD: ALL" > /etc/sudoers.d/build

# import and apply config patches
RUN mkdir /etc/serene

ADD pacman.conf /etc/serene/pacman.conf
ADD makepkg.conf /etc/serene/makepkg.conf

RUN cat /etc/serene/pacman.conf >> /etc/pacman.conf
RUN cat /etc/serene/makepkg.conf >> /etc/makepkg.conf

RUN mkdir -p /app/build
RUN mkdir -p /app/target

# create custom repo file so it can be included
RUN touch /app/build/custom-repo

# update pacman and install packages
RUN pacman -Suy --noconfirm
RUN pacman -Sy --noconfirm git

# create build folder and script
ADD scripts/* /app
RUN chmod +x /app/*.sh

# switch to builder user
USER build
WORKDIR /app