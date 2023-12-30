FROM archlinux:base-devel

# create build user
RUN useradd -m build && echo "build ALL=(ALL) NOPASSWD: ALL" > /etc/sudoers.d/build

# import and apply config patches
RUN mkdir /etc/serene

ADD pacman.conf /etc/serene/pacman.conf
ADD makepkg.conf /etc/serene/makepkg.conf

RUN cat /etc/serene/pacman.conf >> /etc/pacman.conf
RUN cat /etc/serene/makepkg.conf >> /etc/makepkg.conf

# update pacman and install packages
RUN pacman -Suy --noconfirm
RUN pacman -Sy --noconfirm git

# create build folder and script
ADD run.sh /app/run.sh
RUN mkdir /app/build
RUN mkdir /app/target

# switch to builder user
USER build
WORKDIR /app

# run
CMD sh run.sh