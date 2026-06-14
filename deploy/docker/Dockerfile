# syntax=docker/dockerfile:1
#
# 多阶段构建：cargo-chef 缓存依赖编译，显著加速重建。
# 通过构建参数选择复杂度档：
#   docker build -f deploy/docker/Dockerfile --build-arg DMS_FEATURES=full -t dms-server .
#   docker build -f deploy/docker/Dockerfile -t dms-server .            # 精简档（默认）
#
# 注：迁移 SQL 由 sqlx::migrate! 在编译期内嵌进二进制，运行时无需 migrations/ 目录；
# 运行时仅需 config/（figment 分层读取）。本镜像不依赖活库即可构建（运行时查询）。

ARG RUST_VERSION=1.94

# ---- 准备 cargo-chef ----
FROM rust:${RUST_VERSION}-slim-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

# ---- 生成依赖配方 ----
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---- 构建 ----
FROM chef AS builder
ARG DMS_FEATURES=""
COPY --from=planner /app/recipe.json recipe.json
# 先 cook 依赖（仅当 Cargo.toml/锁文件变化才失效）。
RUN cargo chef cook --release --recipe-path recipe.json -p dms-server --features "${DMS_FEATURES}"
# 再编译本项目源码。
COPY . .
RUN cargo build --release --locked -p dms-server --features "${DMS_FEATURES}"

# ---- 运行时（精简、非 root）----
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN useradd --system --uid 10001 --no-create-home dms
WORKDIR /app
COPY --from=builder /app/target/release/dms-server /usr/local/bin/dms-server
COPY config ./config
USER dms
EXPOSE 8080
ENTRYPOINT ["dms-server"]
