-- 索引优化
-- 任务表索引
CREATE INDEX idx_tasks_schedule_status ON tasks(schedule, status) WHERE status = 'ACTIVE';
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_created_at ON tasks(created_at);
CREATE INDEX idx_tasks_updated_at ON tasks(updated_at);

-- 任务执行实例表索引
CREATE INDEX idx_task_runs_status_worker ON task_runs(status, worker_id);
CREATE INDEX idx_task_runs_task_id_status ON task_runs(task_id, status);
CREATE INDEX idx_task_runs_scheduled_at ON task_runs(scheduled_at);
CREATE INDEX idx_task_runs_status_scheduled ON task_runs(status, scheduled_at) WHERE status IN ('PENDING', 'DISPATCHED');
CREATE INDEX idx_task_runs_worker_status ON task_runs(worker_id, status) WHERE worker_id IS NOT NULL;
CREATE INDEX idx_task_runs_created_at ON task_runs(created_at);

-- Worker节点表索引
CREATE INDEX idx_workers_status_heartbeat ON workers(status, last_heartbeat);
CREATE INDEX idx_workers_status ON workers(status);
CREATE INDEX idx_workers_heartbeat ON workers(last_heartbeat);

-- 任务依赖关系表索引
CREATE INDEX idx_task_dependencies_task_id ON task_dependencies(task_id);
CREATE INDEX idx_task_dependencies_depends_on ON task_dependencies(depends_on_task_id);

-- 任务锁表索引
CREATE INDEX idx_task_locks_expires_at ON task_locks(expires_at);
CREATE INDEX idx_task_locks_locked_by ON task_locks(locked_by);

-- 复合索引用于常见查询模式
CREATE INDEX idx_task_runs_task_retry ON task_runs(task_id, retry_count);
CREATE INDEX idx_task_runs_status_created ON task_runs(status, created_at);