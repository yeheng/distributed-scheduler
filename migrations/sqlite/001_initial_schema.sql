-- SQLite初始数据库架构
-- 创建任务定义表
CREATE TABLE tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    task_type TEXT NOT NULL,
    schedule TEXT NOT NULL,
    parameters TEXT NOT NULL DEFAULT '{}', -- JSON as TEXT in SQLite
    timeout_seconds INTEGER NOT NULL DEFAULT 300,
    max_retries INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'ACTIVE',
    dependencies TEXT DEFAULT '[]', -- JSON array as TEXT in SQLite
    shard_config TEXT, -- JSON as TEXT in SQLite
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    CHECK (status IN ('ACTIVE', 'INACTIVE')),
    CHECK (timeout_seconds > 0),
    CHECK (max_retries >= 0)
);

-- 创建任务执行实例表
CREATE TABLE task_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'PENDING',
    worker_id TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,
    shard_index INTEGER,
    shard_total INTEGER,
    scheduled_at DATETIME NOT NULL,
    started_at DATETIME,
    completed_at DATETIME,
    result TEXT,
    error_message TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
    CHECK (status IN ('PENDING', 'DISPATCHED', 'RUNNING', 'COMPLETED', 'FAILED', 'TIMEOUT')),
    CHECK (retry_count >= 0),
    CHECK (
        (shard_index IS NULL AND shard_total IS NULL) OR 
        (shard_index IS NOT NULL AND shard_total IS NOT NULL AND shard_index >= 0 AND shard_total > 0 AND shard_index < shard_total)
    ),
    CHECK (
        (started_at IS NULL OR started_at >= scheduled_at) AND
        (completed_at IS NULL OR (started_at IS NOT NULL AND completed_at >= started_at))
    )
);

-- 创建Worker节点表
CREATE TABLE workers (
    id TEXT PRIMARY KEY,
    hostname TEXT NOT NULL,
    ip_address TEXT NOT NULL,
    supported_task_types TEXT NOT NULL, -- JSON array as TEXT in SQLite
    max_concurrent_tasks INTEGER NOT NULL,
    current_task_count INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'ALIVE',
    last_heartbeat DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    registered_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    CHECK (status IN ('ALIVE', 'DOWN')),
    CHECK (max_concurrent_tasks > 0),
    CHECK (current_task_count >= 0)
);

-- 创建任务依赖关系表（用于更复杂的依赖管理）
CREATE TABLE task_dependencies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id INTEGER NOT NULL,
    depends_on_task_id INTEGER NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
    FOREIGN KEY (depends_on_task_id) REFERENCES tasks(id) ON DELETE CASCADE,
    CHECK (task_id != depends_on_task_id),
    UNIQUE(task_id, depends_on_task_id)
);

-- 创建任务执行锁表（用于防止重复调度）
CREATE TABLE task_locks (
    task_id INTEGER PRIMARY KEY,
    locked_by TEXT NOT NULL,
    locked_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at DATETIME NOT NULL,
    
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
);

-- 创建JWT令牌黑名单表
CREATE TABLE jwt_token_blacklist (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    jti TEXT NOT NULL UNIQUE, -- JWT令牌的唯一标识符
    user_id TEXT NOT NULL,     -- 用户ID
    token_type TEXT NOT NULL,   -- 令牌类型: 'access' 或 'refresh'
    revoked_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP, -- 撤销时间
    expires_at DATETIME NOT NULL,   -- 令牌过期时间
    reason TEXT,                        -- 撤销原因
    created_by TEXT,           -- 撤销操作者
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    CHECK (token_type IN ('access', 'refresh')),
    CHECK (expires_at > revoked_at)
);

-- 创建系统状态表（用于嵌入式系统状态管理）
CREATE TABLE system_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);