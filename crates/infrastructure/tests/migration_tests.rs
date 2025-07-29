use sqlx::PgPool;
use testcontainers::runners::SyncRunner;
use testcontainers_modules::postgres::Postgres;

/// 测试数据库迁移功能
async fn setup_empty_database() -> (testcontainers::Container<Postgres>, PgPool) {
    let postgres_image = Postgres::default()
        .with_db_name("scheduler_migration_test")
        .with_user("test_user")
        .with_password("test_password");

    let container = postgres_image.start().unwrap();
    let connection_string = format!(
        "postgresql://test_user:test_password@127.0.0.1:{}/scheduler_migration_test",
        container.get_host_port_ipv4(5432).unwrap()
    );

    let pool = PgPool::connect(&connection_string).await.unwrap();
    (container, pool)
}

#[tokio::test]
async fn test_database_migrations_from_scratch() {
    let (_container, pool) = setup_empty_database().await;

    // 运行迁移
    sqlx::migrate!("../../migrations").run(&pool).await.unwrap();

    // 验证迁移表存在
    let migration_table_exists: (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = '_sqlx_migrations')"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(migration_table_exists.0);

    // 验证所有核心表都被创建
    let tables_query = r#"
        SELECT table_name 
        FROM information_schema.tables 
        WHERE table_schema = 'public' AND table_name != '_sqlx_migrations'
        ORDER BY table_name
    "#;

    let tables: Vec<(String,)> = sqlx::query_as(tables_query).fetch_all(&pool).await.unwrap();

    let table_names: Vec<String> = tables.into_iter().map(|(name,)| name).collect();

    // 验证所有必要的表都存在
    assert!(table_names.contains(&"tasks".to_string()));
    assert!(table_names.contains(&"task_runs".to_string()));
    assert!(table_names.contains(&"workers".to_string()));

    println!("Created tables: {:?}", table_names);
}

#[tokio::test]
async fn test_migration_idempotency() {
    let (_container, pool) = setup_empty_database().await;

    // 第一次运行迁移
    sqlx::migrate!("../../migrations").run(&pool).await.unwrap();

    // 获取第一次迁移后的表数量
    let first_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // 第二次运行迁移（应该是幂等的）
    sqlx::migrate!("../../migrations").run(&pool).await.unwrap();

    // 获取第二次迁移后的表数量
    let second_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // 表数量应该相同
    assert_eq!(first_count.0, second_count.0);
}

#[tokio::test]
async fn test_table_constraints_and_indexes() {
    let (_container, pool) = setup_empty_database().await;

    // 运行迁移
    sqlx::migrate!("../../migrations").run(&pool).await.unwrap();

    // 验证主键约束
    let pk_constraints: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT tc.table_name, tc.constraint_name
        FROM information_schema.table_constraints tc
        WHERE tc.constraint_type = 'PRIMARY KEY' 
        AND tc.table_schema = 'public'
        ORDER BY tc.table_name
    "#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let pk_tables: Vec<String> = pk_constraints
        .iter()
        .map(|(table, _)| table.clone())
        .collect();
    assert!(pk_tables.contains(&"tasks".to_string()));
    assert!(pk_tables.contains(&"task_runs".to_string()));
    assert!(pk_tables.contains(&"workers".to_string()));

    // 验证外键约束
    let fk_constraints: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT tc.table_name, tc.constraint_name
        FROM information_schema.table_constraints tc
        WHERE tc.constraint_type = 'FOREIGN KEY' 
        AND tc.table_schema = 'public'
        ORDER BY tc.table_name
    "#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    // task_runs表应该有指向tasks表的外键
    assert!(fk_constraints.iter().any(|(table, _)| table == "task_runs"));

    // 验证索引存在
    let indexes: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT schemaname, indexname
        FROM pg_indexes
        WHERE schemaname = 'public'
        ORDER BY indexname
    "#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let index_names: Vec<String> = indexes.iter().map(|(_, name)| name.clone()).collect();

    // 验证一些关键索引存在（具体索引名称可能因迁移文件而异）
    assert!(index_names
        .iter()
        .any(|name| name.contains("tasks") && name.contains("pkey")));
    assert!(index_names
        .iter()
        .any(|name| name.contains("task_runs") && name.contains("pkey")));
    assert!(index_names
        .iter()
        .any(|name| name.contains("workers") && name.contains("pkey")));

    println!("Found indexes: {:?}", index_names);
}
