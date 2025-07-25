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
