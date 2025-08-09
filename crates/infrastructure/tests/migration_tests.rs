use sqlx::PgPool;
use testcontainers::{runners::AsyncRunner, ImageExt};
use testcontainers_modules::postgres::Postgres;

async fn setup_empty_database() -> (testcontainers::ContainerAsync<Postgres>, PgPool) {
    let postgres_image = Postgres::default()
        .with_db_name("scheduler_migration_test")
        .with_user("test_user")
        .with_password("test_password")
        .with_tag("16-alpine");

    let container = postgres_image.start().await.unwrap();
    let connection_string = format!(
        "postgresql://test_user:test_password@127.0.0.1:{}/scheduler_migration_test",
        container.get_host_port_ipv4(5432).await.unwrap()
    );

    let pool = PgPool::connect(&connection_string).await.unwrap();
    (container, pool)
}

#[tokio::test]
async fn test_database_migrations_from_scratch() {
    let (_container, pool) = setup_empty_database().await;
    sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
    let migration_table_exists: (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = '_sqlx_migrations')"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(migration_table_exists.0);
    let tables_query = r#"
        SELECT table_name 
        FROM information_schema.tables 
        WHERE table_schema = 'public' AND table_name != '_sqlx_migrations'
        ORDER BY table_name
    "#;

    let tables: Vec<(String,)> = sqlx::query_as(tables_query).fetch_all(&pool).await.unwrap();

    let table_names: Vec<String> = tables.into_iter().map(|(name,)| name).collect();
    assert!(table_names.contains(&"tasks".to_string()));
    assert!(table_names.contains(&"task_runs".to_string()));
    assert!(table_names.contains(&"workers".to_string()));

    println!("Created tables: {:?}", table_names);
}

#[tokio::test]
async fn test_migration_idempotency() {
    let (_container, pool) = setup_empty_database().await;
    sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
    let first_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
    let second_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(first_count.0, second_count.0);
}

#[tokio::test]
async fn test_table_constraints_and_indexes() {
    let (_container, pool) = setup_empty_database().await;
    sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
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
    assert!(fk_constraints.iter().any(|(table, _)| table == "task_runs"));
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
