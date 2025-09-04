-- SQLite触发器和额外约束
-- 创建触发器来自动更新updated_at字段

-- 任务表的updated_at触发器
CREATE TRIGGER trigger_tasks_updated_at
    AFTER UPDATE ON tasks
    FOR EACH ROW
    WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE tasks SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- 系统状态表的updated_at触发器
CREATE TRIGGER trigger_system_state_updated_at
    AFTER UPDATE ON system_state
    FOR EACH ROW
    WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE system_state SET updated_at = CURRENT_TIMESTAMP WHERE key = NEW.key;
END;

-- 创建视图来简化常见查询
-- 活跃任务视图
CREATE VIEW active_tasks AS
SELECT 
    id,
    name,
    task_type,
    schedule,
    parameters,
    timeout_seconds,
    max_retries,
    dependencies,
    shard_config,
    created_at,
    updated_at
FROM tasks 
WHERE status = 'ACTIVE';

-- 待执行任务运行视图
CREATE VIEW pending_task_runs AS
SELECT 
    tr.id,
    tr.task_id,
    tr.status,
    tr.worker_id,
    tr.retry_count,
    tr.shard_index,
    tr.shard_total,
    tr.scheduled_at,
    tr.created_at,
    t.name as task_name,
    t.task_type,
    t.timeout_seconds,
    t.max_retries
FROM task_runs tr
JOIN tasks t ON tr.task_id = t.id
WHERE tr.status IN ('PENDING', 'DISPATCHED')
ORDER BY tr.scheduled_at ASC;

-- 活跃Worker视图
CREATE VIEW active_workers AS
SELECT 
    id,
    hostname,
    ip_address,
    supported_task_types,
    max_concurrent_tasks,
    current_task_count,
    last_heartbeat,
    registered_at,
    (max_concurrent_tasks - current_task_count) as available_capacity
FROM workers 
WHERE status = 'ALIVE'
ORDER BY available_capacity DESC;

-- 任务执行统计视图
CREATE VIEW task_execution_stats AS
SELECT 
    t.id,
    t.name,
    t.task_type,
    COUNT(tr.id) as total_runs,
    COUNT(CASE WHEN tr.status = 'COMPLETED' THEN 1 END) as successful_runs,
    COUNT(CASE WHEN tr.status = 'FAILED' THEN 1 END) as failed_runs,
    COUNT(CASE WHEN tr.status = 'TIMEOUT' THEN 1 END) as timeout_runs,
    AVG(CASE 
        WHEN tr.status = 'COMPLETED' AND tr.started_at IS NOT NULL AND tr.completed_at IS NOT NULL 
        THEN (julianday(tr.completed_at) - julianday(tr.started_at)) * 86400 
    END) as avg_execution_time_seconds,
    MAX(tr.created_at) as last_run_at
FROM tasks t
LEFT JOIN task_runs tr ON t.id = tr.task_id
GROUP BY t.id, t.name, t.task_type;