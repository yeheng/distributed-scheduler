#[cfg(test)]
mod cron_utils_tests {
    use scheduler_dispatcher::cron_utils::*;

    use chrono::{Duration, Utc};
    use chrono::{TimeZone, Timelike};

    #[test]
    fn test_cron_scheduler_creation() {
        let scheduler = CronScheduler::new("0 0 0 * * *");
        assert!(scheduler.is_ok());
        let scheduler = CronScheduler::new("invalid");
        assert!(scheduler.is_err());
    }

    #[test]
    fn test_should_trigger() {
        let scheduler = CronScheduler::new("0 * * * * *").unwrap();

        let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let _next_minute = Utc.with_ymd_and_hms(2024, 1, 1, 12, 1, 0).unwrap();
        let next_minute_plus = Utc.with_ymd_and_hms(2024, 1, 1, 12, 1, 30).unwrap();
        assert!(scheduler.should_trigger(Some(base_time), next_minute_plus));
        let same_minute = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 30).unwrap();
        assert!(!scheduler.should_trigger(Some(base_time), same_minute));
    }

    #[test]
    fn test_next_execution_time() {
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
        assert!(CronScheduler::validate_cron_expression("0 0 0 * * *").is_ok());
        assert!(CronScheduler::validate_cron_expression("0 */5 * * * *").is_ok());
        assert!(CronScheduler::validate_cron_expression("0 0 9-17 * * 1-5").is_ok());
        assert!(CronScheduler::validate_cron_expression("invalid").is_err());
        assert!(CronScheduler::validate_cron_expression("0 0 0 32 * *").is_err());
        assert!(CronScheduler::validate_cron_expression("").is_err());
    }

    #[test]
    fn test_upcoming_times() {
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
        let scheduler = CronScheduler::new("0 * * * * *").unwrap();

        let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let now = Utc.with_ymd_and_hms(2024, 1, 1, 12, 5, 0).unwrap(); // 5分钟后
        assert!(scheduler.is_task_overdue(Some(base_time), now, 2));
        assert!(!scheduler.is_task_overdue(Some(base_time), now, 5));
    }

    #[test]
    fn test_get_frequency_description() {
        let scheduler = CronScheduler::new("0 * * * * *").unwrap();
        let desc = scheduler.get_frequency_description();
        assert!(desc.contains("分钟") || desc.contains("60秒"));
        let scheduler = CronScheduler::new("0 0 * * * *").unwrap();
        let desc = scheduler.get_frequency_description();
        assert!(desc.contains("小时") || desc.contains("3600秒"));
        let scheduler = CronScheduler::new("0 0 0 * * *").unwrap();
        let desc = scheduler.get_frequency_description();
        assert!(desc.contains("天") || desc.contains("小时"));
    }

    #[test]
    fn test_time_until_next_execution() {
        let scheduler = CronScheduler::new("0 0 * * * *").unwrap();

        let now = Utc.with_ymd_and_hms(2024, 1, 1, 12, 30, 0).unwrap();
        let time_until = scheduler.time_until_next_execution(now);

        assert!(time_until.is_some());
        let duration = time_until.unwrap();
        assert_eq!(duration.num_minutes(), 30); // 距离下一个整点还有30分钟
    }

    #[test]
    fn test_will_execute_within() {
        let scheduler = CronScheduler::new("0 0 * * * *").unwrap();

        let now = Utc.with_ymd_and_hms(2024, 1, 1, 12, 30, 0).unwrap();
        assert!(scheduler.will_execute_within(now, Duration::hours(1)));
        assert!(!scheduler.will_execute_within(now, Duration::minutes(10)));
    }

    #[test]
    fn test_should_trigger_with_logging() {
        let scheduler = CronScheduler::new("0 * * * * *").unwrap();

        let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let trigger_time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 1, 30).unwrap();
        assert!(scheduler.should_trigger(Some(base_time), trigger_time));
        let first_trigger_time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 30).unwrap();
        assert!(scheduler.should_trigger(None, first_trigger_time));
    }

    #[test]
    fn test_complex_cron_expressions() {
        let simple_scheduler = CronScheduler::new("0 * * * * *").unwrap();

        let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let trigger_time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 1, 30).unwrap();
        assert!(simple_scheduler.should_trigger(Some(base_time), trigger_time));
        let daily_scheduler = CronScheduler::new("0 0 9 * * *").unwrap();
        let yesterday_9am = Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap();
        let today_9_30am = Utc.with_ymd_and_hms(2024, 1, 2, 9, 30, 0).unwrap(); // 第二天
        assert!(daily_scheduler.should_trigger(Some(yesterday_9am), today_9_30am));
        let today_9am = Utc.with_ymd_and_hms(2024, 1, 2, 9, 0, 0).unwrap();
        assert!(!daily_scheduler.should_trigger(Some(today_9am), today_9_30am));
    }

    #[test]
    fn test_edge_cases() {
        let scheduler = CronScheduler::new("* * * * * *").unwrap();

        let base_time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let one_second_later = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 1).unwrap();
        assert!(scheduler.should_trigger(Some(base_time), one_second_later));
        let much_later = Utc.with_ymd_and_hms(2024, 1, 1, 12, 5, 0).unwrap();
        assert!(scheduler.is_task_overdue(Some(base_time), much_later, 1)); // 1分钟宽限期
    }
}
