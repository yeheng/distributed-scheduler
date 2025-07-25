use chrono::{DateTime, Duration, Utc};
use cron::Schedule;
use std::str::FromStr;
use tracing::{debug, warn};

use scheduler_core::{Result, SchedulerError};

/// CRON表达式解析和调度工具
pub struct CronScheduler {
    schedule: Schedule,
}

impl CronScheduler {
    /// 创建新的CRON调度器
    pub fn new(cron_expr: &str) -> Result<Self> {
        let schedule = Schedule::from_str(cron_expr).map_err(|e| SchedulerError::InvalidCron {
            expr: cron_expr.to_string(),
            message: e.to_string(),
        })?;

        Ok(Self { schedule })
    }

    /// 检查给定时间是否应该触发任务
    pub fn should_trigger(&self, last_run: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
        match last_run {
            Some(last) => {
                // 从上次执行时间之后开始查找下一次执行时间
                if let Some(next_time) = self.schedule.after(&last).next() {
                    let should_trigger = next_time <= now;
                    if should_trigger {
                        debug!(
                            "任务应该触发: 上次执行={}, 下次执行={}, 当前时间={}",
                            last.format("%Y-%m-%d %H:%M:%S UTC"),
                            next_time.format("%Y-%m-%d %H:%M:%S UTC"),
                            now.format("%Y-%m-%d %H:%M:%S UTC")
                        );
                    }
                    should_trigger
                } else {
                    warn!(
                        "无法计算下一次执行时间，上次执行时间: {}",
                        last.format("%Y-%m-%d %H:%M:%S UTC")
                    );
                    false
                }
            }
            None => {
                // 如果从未执行过，检查当前时间是否匹配CRON表达式
                // 我们需要检查从一个较早的时间点开始，看是否有执行时间已经到了
                let check_from = now - Duration::minutes(1);
                if let Some(next_time) = self.schedule.after(&check_from).next() {
                    let should_trigger = next_time <= now;
                    if should_trigger {
                        debug!(
                            "首次任务应该触发: 下次执行={}, 当前时间={}",
                            next_time.format("%Y-%m-%d %H:%M:%S UTC"),
                            now.format("%Y-%m-%d %H:%M:%S UTC")
                        );
                    }
                    should_trigger
                } else {
                    warn!("无法计算首次执行时间");
                    false
                }
            }
        }
    }

    /// 获取下一次执行时间
    pub fn next_execution_time(&self, from: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.schedule.after(&from).next()
    }

    /// 获取从指定时间开始的多个执行时间
    pub fn upcoming_times(&self, from: DateTime<Utc>, count: usize) -> Vec<DateTime<Utc>> {
        self.schedule.after(&from).take(count).collect()
    }

    /// 验证CRON表达式是否有效
    pub fn validate_cron_expression(cron_expr: &str) -> Result<()> {
        Schedule::from_str(cron_expr).map_err(|e| SchedulerError::InvalidCron {
            expr: cron_expr.to_string(),
            message: e.to_string(),
        })?;
        Ok(())
    }

    /// 检查任务是否已过期（超过预期执行时间太久）
    pub fn is_task_overdue(
        &self,
        last_run: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
        grace_period_minutes: i64,
    ) -> bool {
        match last_run {
            Some(last) => {
                if let Some(expected_time) = self.schedule.after(&last).next() {
                    let grace_period = Duration::minutes(grace_period_minutes);
                    let overdue_threshold = expected_time + grace_period;
                    now > overdue_threshold
                } else {
                    false
                }
            }
            None => {
                // 对于从未执行的任务，检查是否有很久之前就应该执行的时间点
                let check_from = now - Duration::hours(24); // 检查过去24小时
                if let Some(expected_time) = self.schedule.after(&check_from).next() {
                    let grace_period = Duration::minutes(grace_period_minutes);
                    let overdue_threshold = expected_time + grace_period;
                    now > overdue_threshold && expected_time < now
                } else {
                    false
                }
            }
        }
    }

    /// 获取任务的执行频率描述
    pub fn get_frequency_description(&self) -> String {
        // 这是一个简化的实现，实际中可能需要更复杂的逻辑来解析CRON表达式
        let upcoming = self.upcoming_times(Utc::now(), 2);
        if upcoming.len() >= 2 {
            let interval = upcoming[1] - upcoming[0];
            let seconds = interval.num_seconds();

            match seconds {
                s if s < 60 => format!("每{s}秒"),
                s if s < 3600 => format!("每{}分钟", s / 60),
                s if s < 86400 => format!("每{}小时", s / 3600),
                s if s < 604800 => format!("每{}天", s / 86400),
                s => format!("每{}周", s / 604800),
            }
        } else {
            "无法确定频率".to_string()
        }
    }

    /// 计算下次执行时间距离现在的时长
    pub fn time_until_next_execution(&self, now: DateTime<Utc>) -> Option<Duration> {
        self.schedule.after(&now).next().map(|next| next - now)
    }

    /// 检查CRON表达式是否会在合理的时间内执行
    pub fn will_execute_within(&self, from: DateTime<Utc>, duration: Duration) -> bool {
        if let Some(next_time) = self.schedule.after(&from).next() {
            next_time <= from + duration
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
