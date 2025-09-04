-- SQLite Schema Validation Script
-- This script can be used to validate that all tables and indexes are created correctly

-- Check that all expected tables exist
SELECT 
    name,
    type,
    sql
FROM sqlite_master 
WHERE type = 'table' 
ORDER BY name;

-- Check that all expected indexes exist
SELECT 
    name,
    tbl_name,
    sql
FROM sqlite_master 
WHERE type = 'index' 
AND name NOT LIKE 'sqlite_%'
ORDER BY tbl_name, name;

-- Check that all expected views exist
SELECT 
    name,
    sql
FROM sqlite_master 
WHERE type = 'view'
ORDER BY name;

-- Check that all expected triggers exist
SELECT 
    name,
    tbl_name,
    sql
FROM sqlite_master 
WHERE type = 'trigger'
ORDER BY tbl_name, name;

-- Verify foreign key constraints are enabled
PRAGMA foreign_keys;

-- Check table schemas
.schema tasks
.schema task_runs
.schema workers
.schema task_dependencies
.schema task_locks
.schema jwt_token_blacklist
.schema system_state

-- Test basic operations on each table
-- These should not fail if the schema is correct

-- Test tasks table
INSERT INTO tasks (name, task_type, schedule) 
VALUES ('test_task', 'shell', '0 0 * * *');

SELECT COUNT(*) as task_count FROM tasks;

-- Test task_runs table
INSERT INTO task_runs (task_id, scheduled_at) 
VALUES (1, datetime('now'));

SELECT COUNT(*) as task_run_count FROM task_runs;

-- Test workers table
INSERT INTO workers (id, hostname, ip_address, supported_task_types, max_concurrent_tasks) 
VALUES ('test_worker', 'localhost', '127.0.0.1', '["shell", "http"]', 5);

SELECT COUNT(*) as worker_count FROM workers;

-- Test system_state table
INSERT OR REPLACE INTO system_state (key, value) 
VALUES ('test_key', 'test_value');

SELECT COUNT(*) as system_state_count FROM system_state;

-- Clean up test data
DELETE FROM task_runs WHERE task_id = 1;
DELETE FROM tasks WHERE name = 'test_task';
DELETE FROM workers WHERE id = 'test_worker';
DELETE FROM system_state WHERE key = 'test_key';

-- Verify cleanup
SELECT 
    (SELECT COUNT(*) FROM tasks) as tasks,
    (SELECT COUNT(*) FROM task_runs) as task_runs,
    (SELECT COUNT(*) FROM workers) as workers,
    (SELECT COUNT(*) FROM system_state WHERE key != 'schema_version' AND key != 'last_cleanup' AND key != 'system_initialized' AND key != 'embedded_mode') as system_state;

PRAGMA integrity_check;