# 多阶段构建 Dockerfile for 分布式任务调度系统

# ============================================================================
# 构建阶段 - 使用官方 Rust 镜像进行编译
# ============================================================================
FROM rust:1.75-slim-bullseye AS builder

# 设置工作目录
WORKDIR /app

# 安装构建依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# 复制 Cargo 配置文件
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

# 创建一个虚拟的 main.rs 来缓存依赖
RUN mkdir -p src && echo "fn main() {}" > src/main.rs

# 构建依赖（这一层会被缓存）
RUN cargo build --release && rm -rf src/

# 复制源代码
COPY src/ ./src/
COPY config/ ./config/
COPY migrations/ ./migrations/

# 构建应用程序
RUN cargo build --release --bin scheduler

# 验证二进制文件存在
RUN ls -la target/release/

# ============================================================================
# 运行时阶段 - 使用轻量级基础镜像
# ============================================================================
FROM debian:bullseye-slim AS runtime

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    libpq5 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# 创建非 root 用户
RUN groupadd -r scheduler && useradd -r -g scheduler scheduler

# 设置工作目录
WORKDIR /app

# 从构建阶段复制二进制文件
COPY --from=builder /app/target/release/scheduler /usr/local/bin/scheduler

# 复制配置文件和迁移文件
COPY --from=builder /app/config/ ./config/
COPY --from=builder /app/migrations/ ./migrations/

# 创建日志目录
RUN mkdir -p /var/log/scheduler && chown -R scheduler:scheduler /var/log/scheduler

# 创建数据目录
RUN mkdir -p /var/lib/scheduler && chown -R scheduler:scheduler /var/lib/scheduler

# 设置文件权限
RUN chown -R scheduler:scheduler /app
RUN chmod +x /usr/local/bin/scheduler

# 切换到非 root 用户
USER scheduler

# 暴露端口
EXPOSE 8080

# 健康检查
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/api/health || exit 1

# 设置环境变量
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1

# 默认启动命令
CMD ["scheduler", "--config", "config/scheduler.toml", "--mode", "all"]