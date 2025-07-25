#[cfg(test)]
mod cron_utils_tests {
    use crate::cron_utils::*;

    use chrono::{Duration, Utc};
    use chrono::{TimeZone, Timelike};

    #[test]
    fn test_cron_scheduler_creation() {
        // 测试有效的CRON表达式 (6字段格式: 秒 分 时 日 月 周)
        let scheduler = CronScheduler::new("0 0 0 * * *");
        assert!(scheduler.is_ok());

        // 测试无效的CRON表达式
        let scheduler = CronScheduler::new("invalid");
        assert!(scheduler.is_err());
    }

    #[test]
    fn test_should_trigger() {
        // 每分钟执行一次的CRON表达式 (6字段格式: 秒 分 时 日 月 周)
        let scheduler = CronScheduler::new("0 * * * * *").unwrap();

        let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let _next_minute = Utc.with_ymd_and_hms(2024, 1, 1, 12, 1, 0).unwrap();
        let next_minute_plus = Utc.with_ymd_and_hms(2024, 1, 1, 12, 1, 30).unwrap();

        // 在下一分钟应该触发
        assert!(scheduler.should_trigger(Some(base_time), next_minute_plus));

        // 在同一分钟内不应该触发
        let same_minute = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 30).unwrap();
        assert!(!scheduler.should_trigger(Some(base_time), same_minute));
    }

    #[test]
    fn test_next_execution_time() {
        // 每天午夜执行 (6字段格式: 秒 分 时 日 月 周)
        let scheduler = CronScheduler::new("0 0 0 * * *").unwrap();

        let now = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let next = scheduler.next_execution_time(now);

        assert!(next.is_some());
        let next_time = next.unwrap();
        assert_eq!(next_time.hour(), 0);
        assert_eq!(next_time.minute(), 0);
        assert_eq!(next_time.second(), 0);
    }

    #[test]
    fn test_validate_cron_expression() {
        // 有效表达式 (6字段格式: 秒 分 时 日 月 周)
        assert!(CronScheduler::validate_cron_expression("0 0 0 * * *").is_ok());
        assert!(CronScheduler::validate_cron_expression("0 */5 * * * *").is_ok());
        assert!(CronScheduler::validate_cron_expression("0 0 9-17 * * 1-5").is_ok());

        // 无效表达式
        assert!(CronScheduler::validate_cron_expression("invalid").is_err());
        assert!(CronScheduler::validate_cron_expression("0 0 0 32 * *").is_err());
        assert!(CronScheduler::validate_cron_expression("").is_err());
    }

    #[test]
    fn test_upcoming_times() {
        // 每小时执行一次 (6字段格式: 秒 分 时 日 月 周)
        let scheduler = CronScheduler::new("0 0 * * * *").unwrap();

        let now = Utc.with_ymd_and_hms(2024, 1, 1, 12, 30, 0).unwrap();
        let upcoming = scheduler.upcoming_times(now, 3);

        assert_eq!(upcoming.len(), 3);
        assert_eq!(upcoming[0].hour(), 13);
        assert_eq!(upcoming[1].hour(), 14);
        assert_eq!(upcoming[2].hour(), 15);
    }

    #[test]
    fn test_is_task_overdue() {
        // 每分钟执行一次 (6字段格式: 秒 分 时 日 月 周)
        let scheduler = CronScheduler::new("0 * * * * *").unwrap();

        let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let now = Utc.with_ymd_and_hms(2024, 1, 1, 12, 5, 0).unwrap(); // 5分钟后

        // 任务应该在12:01执行，现在是12:05，超过了2分钟的宽限期
        assert!(scheduler.is_task_overdue(Some(base_time), now, 2));

        // 但在5分钟的宽限期内不算过期
        assert!(!scheduler.is_task_overdue(Some(base_time), now, 5));
    }

    #[test]
    fn test_get_frequency_description() {
        // 每分钟执行
        let scheduler = CronScheduler::new("0 * * * * *").unwrap();
        let desc = scheduler.get_frequency_description();
        assert!(desc.contains("分钟") || desc.contains("60秒"));

        // 每小时执行
        let scheduler = CronScheduler::new("0 0 * * * *").unwrap();
        let desc = scheduler.get_frequency_description();
        assert!(desc.contains("小时") || desc.contains("3600秒"));

        // 每天执行
        let scheduler = CronScheduler::new("0 0 0 * * *").unwrap();
        let desc = scheduler.get_frequency_description();
        assert!(desc.contains("天") || desc.contains("小时"));
    }

    #[test]
    fn test_time_until_next_execution() {
        // 每小时执行一次 (6字段格式: 秒 分 时 日 月 周)
        let scheduler = CronScheduler::new("0 0 * * * *").unwrap();

        let now = Utc.with_ymd_and_hms(2024, 1, 1, 12, 30, 0).unwrap();
        let time_until = scheduler.time_until_next_execution(now);

        assert!(time_until.is_some());
        let duration = time_until.unwrap();
        assert_eq!(duration.num_minutes(), 30); // 距离下一个整点还有30分钟
    }

    #[test]
    fn test_will_execute_within() {
        // 每小时执行一次 (6字段格式: 秒 分 时 日 月 周)
        let scheduler = CronScheduler::new("0 0 * * * *").unwrap();

        let now = Utc.with_ymd_and_hms(2024, 1, 1, 12, 30, 0).unwrap();

        // 在1小时内会执行
        assert!(scheduler.will_execute_within(now, Duration::hours(1)));

        // 在10分钟内不会执行
        assert!(!scheduler.will_execute_within(now, Duration::minutes(10)));
    }

    #[test]
    fn test_should_trigger_with_logging() {
        // 每分钟执行一次的CRON表达式 (6字段格式: 秒 分 时 日 月 周)
        let scheduler = CronScheduler::new("0 * * * * *").unwrap();

        let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let trigger_time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 1, 30).unwrap();

        // 应该触发，因为已经过了下一分钟
        assert!(scheduler.should_trigger(Some(base_time), trigger_time));

        // 测试首次执行的情况
        let first_trigger_time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 30).unwrap();
        assert!(scheduler.should_trigger(None, first_trigger_time));
    }

    #[test]
    fn test_complex_cron_expressions() {
        // 使用一个简单的测试：每分钟执行的CRON表达式
        let simple_scheduler = CronScheduler::new("0 * * * * *").unwrap();

        let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let trigger_time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 1, 30).unwrap();

        // 已经在12:00执行过，现在是12:01:30，应该触发下一分钟的任务
        assert!(simple_scheduler.should_trigger(Some(base_time), trigger_time));

        // 测试每天执行的情况
        let daily_scheduler = CronScheduler::new("0 0 9 * * *").unwrap();
        let yesterday_9am = Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap();
        let today_9_30am = Utc.with_ymd_and_hms(2024, 1, 2, 9, 30, 0).unwrap(); // 第二天

        // 昨天9点执行过，今天9:30应该触发
        assert!(daily_scheduler.should_trigger(Some(yesterday_9am), today_9_30am));

        // 今天9点执行过，今天9:30不应该再次触发
        let today_9am = Utc.with_ymd_and_hms(2024, 1, 2, 9, 0, 0).unwrap();
        assert!(!daily_scheduler.should_trigger(Some(today_9am), today_9_30am));
    }

    #[test]
    fn test_edge_cases() {
        // 每秒执行一次
        let scheduler = CronScheduler::new("* * * * * *").unwrap();

        let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let one_second_later = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap();

        // 应该每秒都触发
        assert!(scheduler.should_trigger(Some(base_time), one_second_later));

        // 测试过期检查
        let much_later = Utc.with_ymd_and_hms(2024, 1, 1, 12, 5, 0).unwrap();
        assert!(scheduler.is_task_overdue(Some(base_time), much_later, 1)); // 1分钟宽限期
    }
}
