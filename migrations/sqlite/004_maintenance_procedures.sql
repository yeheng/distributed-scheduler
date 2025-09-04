-- SQLite维护和清理程序
-- 由于SQLite不支持存储过程，这些是可以定期执行的清理查询

-- 清理过期的JWT令牌黑名单记录
-- 这个查询可以定期执行来清理过期的令牌记录
-- DELETE FROM jwt_token_blacklist WHERE expires_at < datetime('now');

-- 清理过期的任务锁
-- DELETE FROM task_locks WHERE expires_at < datetime('now');

-- 清理旧的任务执行记录（保留最近30天的记录）
-- DELETE FROM task_runs 
-- WHERE created_at < datetime('now', '-30 days') 
-- AND status IN ('COMPLETED', 'FAILED', 'TIMEOUT');

-- 更新Worker心跳状态（将长时间未心跳的Worker标记为DOWN）
-- UPDATE workers 
-- SET status = 'DOWN' 
-- WHERE status = 'ALIVE' 
-- AND last_heartbeat < datetime('now', '-5 minutes');

-- 创建一些有用的查询函数（通过视图实现）

-- 系统健康状态视图
CREATE VIEW system_health AS
SELECT 
    'tasks' as component,
    COUNT(*) as total_count,
    COUNT(CASE WHEN status = 'ACTIVE' THEN 1 END) as active_count
FROM tasks
UNION ALL
SELECT 
    'workers' as component,
    COUNT(*) as total_count,
    COUNT(CASE WHEN status = 'ALIVE' THEN 1 END) as active_count
FROM workers
UNION ALL
SELECT 
    'task_runs_pending' as component,
    COUNT(*) as total_count,
    COUNT(CASE WHEN status = 'PENDING' THEN 1 END) as active_count
FROM task_runs
WHERE created_at > datetime('now', '-1 day')
UNION ALL
SELECT 
    'task_runs_running' as component,
    COUNT(*) as total_count,
    COUNT(CASE WHEN status = 'RUNNING' THEN 1 END) as active_count
FROM task_runs
WHERE created_at > datetime('now', '-1 day');

-- 任务依赖关系检查视图
CREATE VIEW task_dependency_check AS
SELECT 
    td.task_id,
    t1.name as task_name,
    td.depends_on_task_id,
    t2.name as depends_on_task_name,
    t1.status as task_status,
    t2.status as dependency_status,
    CASE 
        WHEN t2.status = 'INACTIVE' THEN 'WARNING: Dependency is inactive'
        WHEN t2.id IS NULL THEN 'ERROR: Dependency does not exist'
        ELSE 'OK'
    END as dependency_check
FROM task_dependencies td
JOIN tasks t1 ON td.task_id = t1.id
LEFT JOIN tasks t2 ON td.depends_on_task_id = t2.id;

-- 性能监控视图
CREATE VIEW performance_metrics AS
SELECT 
    'avg_task_execution_time' as metric_name,
    printf('%.2f', AVG(
        CASE 
            WHEN tr.status = 'COMPLETED' AND tr.started_at IS NOT NULL AND tr.completed_at IS NOT NULL 
            THEN (julianday(tr.completed_at) - julianday(tr.started_at)) * 86400 
        END
    )) as metric_value,
    'seconds' as unit
FROM task_runs tr
WHERE tr.created_at > datetime('now', '-1 day')
UNION ALL
SELECT 
    'task_success_rate' as metric_name,
    printf('%.2f', 
        CAST(COUNT(CASE WHEN status = 'COMPLETED' THEN 1 END) AS REAL) / 
        CAST(COUNT(*) AS REAL) * 100
    ) as metric_value,
    'percent' as unit
FROM task_runs
WHERE created_at > datetime('now', '-1 day')
UNION ALL
SELECT 
    'active_workers' as metric_name,
    CAST(COUNT(*) AS TEXT) as metric_value,
    'count' as unit
FROM workers
WHERE status = 'ALIVE'
UNION ALL
SELECT 
    'pending_tasks' as metric_name,
    CAST(COUNT(*) AS TEXT) as metric_value,
    'count' as unit
FROM task_runs
WHERE status = 'PENDING';

-- 插入一些初始系统状态记录
INSERT OR IGNORE INTO system_state (key, value) VALUES 
    ('schema_version', '1.0.0'),
    ('last_cleanup', datetime('now')),
    ('system_initialized', datetime('now')),
    ('embedded_mode', 'true');