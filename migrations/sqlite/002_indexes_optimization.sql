-- SQLite索引优化
-- 任务表索引
CREATE INDEX idx_tasks_name ON tasks(name);
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_schedule_status ON tasks(schedule, status);
CREATE INDEX idx_tasks_created_at ON tasks(created_at);
CREATE INDEX idx_tasks_updated_at ON tasks(updated_at);
CREATE INDEX idx_tasks_task_type ON tasks(task_type);

-- 任务执行实例表索引
CREATE INDEX idx_task_runs_task_id ON task_runs(task_id);
CREATE INDEX idx_task_runs_status ON task_runs(status);
CREATE INDEX idx_task_runs_worker_id ON task_runs(worker_id);
CREATE INDEX idx_task_runs_status_worker ON task_runs(status, worker_id);
CREATE INDEX idx_task_runs_task_id_status ON task_runs(task_id, status);
CREATE INDEX idx_task_runs_scheduled_at ON task_runs(scheduled_at);
CREATE INDEX idx_task_runs_created_at ON task_runs(created_at);
CREATE INDEX idx_task_runs_task_retry ON task_runs(task_id, retry_count);
CREATE INDEX idx_task_runs_status_created ON task_runs(status, created_at);

-- 为常见的查询模式创建部分索引（SQLite支持WHERE子句的索引）
CREATE INDEX idx_task_runs_pending_dispatched ON task_runs(status, scheduled_at) 
WHERE status IN ('PENDING', 'DISPATCHED');

CREATE INDEX idx_task_runs_with_worker ON task_runs(worker_id, status) 
WHERE worker_id IS NOT NULL;

-- Worker节点表索引
CREATE INDEX idx_workers_status ON workers(status);
CREATE INDEX idx_workers_heartbeat ON workers(last_heartbeat);
CREATE INDEX idx_workers_status_heartbeat ON workers(status, last_heartbeat);
CREATE INDEX idx_workers_hostname ON workers(hostname);

-- 任务依赖关系表索引
CREATE INDEX idx_task_dependencies_task_id ON task_dependencies(task_id);
CREATE INDEX idx_task_dependencies_depends_on ON task_dependencies(depends_on_task_id);

-- 任务锁表索引
CREATE INDEX idx_task_locks_expires_at ON task_locks(expires_at);
CREATE INDEX idx_task_locks_locked_by ON task_locks(locked_by);

-- JWT令牌黑名单表索引
CREATE INDEX idx_jwt_blacklist_jti ON jwt_token_blacklist(jti);
CREATE INDEX idx_jwt_blacklist_user_id ON jwt_token_blacklist(user_id);
CREATE INDEX idx_jwt_blacklist_expires_at ON jwt_token_blacklist(expires_at);
CREATE INDEX idx_jwt_blacklist_token_type ON jwt_token_blacklist(token_type);
CREATE INDEX idx_jwt_blacklist_user_type ON jwt_token_blacklist(user_id, token_type);

-- 系统状态表索引
CREATE INDEX idx_system_state_updated_at ON system_state(updated_at);