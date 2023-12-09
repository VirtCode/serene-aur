FROM archlinux:base-devel

# create build user
RUN useradd -m build && echo "build ALL=(ALL) NOPASSWD: ALL" > /etc/sudoers.d/build

# update pacman and install packages
RUN pacman -Suy --noconfirm
RUN pacman -Sy --noconfirm git

# switch to builder user
USER build
WORKDIR /app/builds/

CMD sudo pacman -Suy --noconfirm && \
    sudo chown -R build . && \
    if cd $ID; then git pull; else git clone $REPOSITORY $ID && cd $ID; fi && \
    makepkg -sf --noconfirm