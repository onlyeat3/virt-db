FROM mcr.microsoft.com/devcontainers/rust:0-1-bullseye
RUN echo '[source.crates-io] \n\
registry = "https://github.com/rust-lang/crates.io-index" \n\
replace-with = "ustc" \n\
[source.ustc] \n\
registry = "git://mirrors.ustc.edu.cn/crates.io-index"' > /usr/local/cargo/config.toml


RUN sed -i s@/archive.ubuntu.com/@/mirrors.aliyun.com/@g /etc/apt/sources.list \
    && sed -i s@/deb.debian.org/@/mirrors.aliyun.com/@g /etc/apt/sources.list \
    && apt-get clean \
    && apt-get update \
    && apt-get install \
    && apt install git wget curl vim -y