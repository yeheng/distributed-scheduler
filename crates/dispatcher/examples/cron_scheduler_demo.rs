use chrono::Utc;
use scheduler_dispatcher::cron_utils::CronScheduler;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== CRON调度器演示 ===\n");

    // 1. 创建不同类型的CRON调度器
    println!("1. 创建CRON调度器:");

    // 每分钟执行
    let every_minute = CronScheduler::new("0 * * * * *")?;
    println!(
        "   每分钟执行: {}",
        every_minute.get_frequency_description()
    );

    // 每小时执行
    let every_hour = CronScheduler::new("0 0 * * * *")?;
    println!("   每小时执行: {}", every_hour.get_frequency_description());

    // 每天上午9点执行
    let daily_9am = CronScheduler::new("0 0 9 * * *")?;
    println!("   每天9点执行: {}", daily_9am.get_frequency_description());

    // 工作日上午9点执行
    let weekday_9am = CronScheduler::new("0 0 9 * * 1-5")?;
    println!(
        "   工作日9点执行: {}",
        weekday_9am.get_frequency_description()
    );

    println!();

    // 2. 演示任务到期检测
    println!("2. 任务到期检测:");
    let now = Utc::now();
    let five_minutes_ago = now - chrono::Duration::minutes(5);

    println!("   当前时间: {}", now.format("%Y-%m-%d %H:%M:%S UTC"));
    println!(
        "   上次执行: {}",
        five_minutes_ago.format("%Y-%m-%d %H:%M:%S UTC")
    );

    let is_overdue = every_minute.is_task_overdue(Some(five_minutes_ago), now, 2);
    println!("   任务是否过期(宽限期2分钟): {is_overdue}");

    println!();

    // 3. 演示下次执行时间计算
    println!("3. 下次执行时间:");
    if let Some(next_time) = every_hour.next_execution_time(now) {
        println!(
            "   每小时任务下次执行: {}",
            next_time.format("%Y-%m-%d %H:%M:%S UTC")
        );
        if let Some(duration) = every_hour.time_until_next_execution(now) {
            println!("   距离下次执行: {}分钟", duration.num_minutes());
        }
    }

    println!();

    // 4. 演示即将到来的执行时间
    println!("4. 即将到来的执行时间:");
    let upcoming = daily_9am.upcoming_times(now, 5);
    for (i, time) in upcoming.iter().enumerate() {
        println!("   第{}次: {}", i + 1, time.format("%Y-%m-%d %H:%M:%S UTC"));
    }

    println!();

    // 5. 演示任务触发检测
    println!("5. 任务触发检测:");
    let last_run = now - chrono::Duration::minutes(65); // 65分钟前
    let should_trigger = every_hour.should_trigger(Some(last_run), now);
    println!("   上次执行: {}", last_run.format("%Y-%m-%d %H:%M:%S UTC"));
    println!("   当前时间: {}", now.format("%Y-%m-%d %H:%M:%S UTC"));
    println!("   是否应该触发: {should_trigger}");

    println!();

    // 6. 演示CRON表达式验证
    println!("6. CRON表达式验证:");
    let valid_expressions = vec![
        "0 * * * * *",      // 每分钟
        "0 0 * * * *",      // 每小时
        "0 0 0 * * *",      // 每天
        "0 0 9-17 * * 1-5", // 工作日9-17点
        "0 */15 * * * *",   // 每15分钟
    ];

    let invalid_expressions = vec![
        "invalid",
        "0 0 25 * * *", // 无效小时
        "0 60 * * * *", // 无效分钟
        "",             // 空表达式
    ];

    println!("   有效的CRON表达式:");
    for expr in &valid_expressions {
        match CronScheduler::validate_cron_expression(expr) {
            Ok(_) => println!("     ✓ {expr}"),
            Err(e) => println!("     ✗ {expr} - {e}"),
        }
    }

    println!("   无效的CRON表达式:");
    for expr in &invalid_expressions {
        match CronScheduler::validate_cron_expression(expr) {
            Ok(_) => println!("     ✓ {expr}"),
            Err(e) => println!("     ✗ {expr} - {e}"),
        }
    }

    println!("\n=== 演示完成 ===");
    Ok(())
}
