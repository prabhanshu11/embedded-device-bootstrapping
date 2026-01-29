# Pi Flasher - Privileged container for SD card operations
# Allows flashing Raspberry Pi OS without repeated sudo prompts
#
# Build: docker build -t pi-flasher flash/
# Run:   docker run --rm -it --privileged -v /dev:/dev -v ~/Downloads:/downloads pi-flasher
#
# Usage inside container:
#   flash-sd --device /dev/mmcblk0 --profile pi-zero-2w --wifi "SSID" "PASS"

FROM archlinux:latest

# Install required tools
RUN pacman -Syu --noconfirm && \
    pacman -S --noconfirm \
    util-linux \
    dosfstools \
    e2fsprogs \
    parted \
    xz \
    curl \
    wget \
    openssl \
    && pacman -Scc --noconfirm

# Create working directories
RUN mkdir -p /mnt/pi-boot /mnt/pi-root /downloads

# Add flash script
COPY flash-sd.sh /usr/local/bin/flash-sd
RUN chmod +x /usr/local/bin/flash-sd

# Use /downloads for image cache inside container
ENV XDG_CACHE_HOME=/downloads

WORKDIR /

CMD ["/bin/bash"]
