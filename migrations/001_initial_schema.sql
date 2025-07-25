-- 初始数据库架构
-- 创建任务定义表
CREATE TABLE tasks (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    task_type VARCHAR(50) NOT NULL,
    schedule VARCHAR(100) NOT NULL,
    parameters JSONB NOT NULL DEFAULT '{}',
    timeout_seconds INTEGER NOT NULL DEFAULT 300,
    max_retries INTEGER NOT NULL DEFAULT 0,
    status VARCHAR(20) NOT NULL DEFAULT 'ACTIVE',
    dependencies BIGINT[] DEFAULT '{}',
    shard_config JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT tasks_status_check CHECK (status IN ('ACTIVE', 'INACTIVE')),
    CONSTRAINT tasks_timeout_positive CHECK (timeout_seconds > 0),
    CONSTRAINT tasks_max_retries_non_negative CHECK (max_retries >= 0)
);

-- 创建任务执行实例表
CREATE TABLE task_runs (
    id BIGSERIAL PRIMARY KEY,
    task_id BIGINT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    status VARCHAR(20) NOT NULL DEFAULT 'PENDING',
    worker_id VARCHAR(255),
    retry_count INTEGER NOT NULL DEFAULT 0,
    shard_index INTEGER,
    shard_total INTEGER,
    scheduled_at TIMESTAMPTZ NOT NULL,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    result TEXT,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT task_runs_status_check CHECK (status IN ('PENDING', 'DISPATCHED', 'RUNNING', 'COMPLETED', 'FAILED', 'TIMEOUT')),
    CONSTRAINT task_runs_retry_count_non_negative CHECK (retry_count >= 0),
    CONSTRAINT task_runs_shard_valid CHECK (
        (shard_index IS NULL AND shard_total IS NULL) OR 
        (shard_index IS NOT NULL AND shard_total IS NOT NULL AND shard_index >= 0 AND shard_total > 0 AND shard_index < shard_total)
    ),
    CONSTRAINT task_runs_timing_check CHECK (
        (started_at IS NULL OR started_at >= scheduled_at) AND
        (completed_at IS NULL OR (started_at IS NOT NULL AND completed_at >= started_at))
    )
);

-- 创建Worker节点表
CREATE TABLE workers (
    id VARCHAR(255) PRIMARY KEY,
    hostname VARCHAR(255) NOT NULL,
    ip_address INET NOT NULL,
    supported_task_types TEXT[] NOT NULL,
    max_concurrent_tasks INTEGER NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'ALIVE',
    last_heartbeat TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    registered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT workers_status_check CHECK (status IN ('ALIVE', 'DOWN')),
    CONSTRAINT workers_max_concurrent_positive CHECK (max_concurrent_tasks > 0)
);

-- 创建任务依赖关系表（用于更复杂的依赖管理）
CREATE TABLE task_dependencies (
    id BIGSERIAL PRIMARY KEY,
    task_id BIGINT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    depends_on_task_id BIGINT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT task_dependencies_no_self_reference CHECK (task_id != depends_on_task_id),
    UNIQUE(task_id, depends_on_task_id)
);

-- 创建任务执行锁表（用于防止重复调度）
CREATE TABLE task_locks (
    task_id BIGINT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    locked_by VARCHAR(255) NOT NULL,
    locked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL
);