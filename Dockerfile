# syntax=docker/dockerfile:1

# Stage 1 Build QEMU
FROM ubuntu:22.04 as build_qemu

ARG QEMU_VERSION=7.0.0

RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y \
        wget build-essential libglib2.0-dev libfdt-dev libpixman-1-dev \
        zlib1g-dev ninja-build pkg-config libdbus-1-dev \
        qemu-system-misc gcc-riscv64-linux-gnu binutils-riscv64-linux-gnu && \
    rm -rf /var/lib/apt/lists/*

RUN wget https://download.qemu.org/qemu-${QEMU_VERSION}.tar.xz && \
    tar xf qemu-${QEMU_VERSION}.tar.xz && \
    cd qemu-${QEMU_VERSION} && \
    ./configure --target-list=riscv64-softmmu,riscv64-linux-user && \
    make -j$(nproc) && \
    make install

# Stage 2 Set Lab Environment
FROM ubuntu:22.04 as build

WORKDIR /tmp

# 2.0. Install general tools
RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y \
        jq curl git python3 wget build-essential \
        libglib2.0-0 libfdt1 libpixman-1-0 zlib1g \
        gdb-multiarch && \
    rm -rf /var/lib/apt/lists/*

# 2.1. Copy qemu
COPY --from=build_qemu /usr/local/bin/ /usr/local/bin/

# 2.2. Install Rust
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static \
    RUSTUP_UPDATE_ROOT=https://mirrors.ustc.edu.cn/rust-static/rustup

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --no-modify-path --profile minimal --default-toolchain nightly

# 2.3. Build env for labs
# 如果构建上下文中存在 rust-toolchain.toml，再取消注释下面这行
# COPY rust-toolchain.toml rust-toolchain.toml

RUN rustup default nightly && \
    rustup target add riscv64gc-unknown-none-elf && \
    rustup component add llvm-tools-preview && \
    rustup component add rust-src && \
    cargo install cargo-binutils --locked

# 2.4. Set GDB
RUN ln -sf /usr/bin/gdb-multiarch /usr/bin/riscv64-unknown-elf-gdb

# Stage 3 Sanity checking
FROM build as test
RUN qemu-system-riscv64 --version && \
    qemu-riscv64 --version && \
    rustup --version && \
    cargo --version && \
    rustc --version && \
    riscv64-unknown-elf-gdb --version

# 将你的程序复制到镜像中（确保构建上下文存在 PsInterTrace 目录）
# COPY PsInterTrace /home/
COPY . /home/
