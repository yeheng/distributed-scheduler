use chrono::{DateTime, Duration, Utc};
use cron::Schedule;
use std::str::FromStr;
use tracing::{debug, warn};

use scheduler_errors::{SchedulerError, SchedulerResult};

#[derive(Debug)]
pub struct CronScheduler {
    schedule: Schedule,
}

impl CronScheduler {
    pub fn new(cron_expr: &str) -> SchedulerResult<Self> {
        let schedule = Schedule::from_str(cron_expr).map_err(|e| SchedulerError::InvalidCron {
            expr: cron_expr.to_string(),
            message: e.to_string(),
        })?;

        Ok(Self { schedule })
    }

    pub fn should_trigger(&self, last_run: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
        match last_run {
            Some(last) => {
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
                    warn!("Cron表达式无法解析下次执行时间");
                    false
                }
            }
            None => {
                if let Some(next_time) = self.schedule.upcoming(Utc).next() {
                    let should_trigger = next_time <= now;
                    if should_trigger {
                        debug!(
                            "任务首次执行: 下次执行={}, 当前时间={}",
                            next_time.format("%Y-%m-%d %H:%M:%S UTC"),
                            now.format("%Y-%m-%d %H:%M:%S UTC")
                        );
                    }
                    should_trigger
                } else {
                    warn!("Cron表达式无法解析下次执行时间");
                    false
                }
            }
        }
    }

    pub fn should_run(&self, _now: DateTime<Utc>) -> bool {
        // 对于从未运行过的任务，检查是否有有效的调度时间
        // 如果有，则认为可以立即执行（避免时间精度问题）
        if let Some(_next_time) = self.schedule.upcoming(Utc).next() {
            true
        } else {
            false
        }
    }

    pub fn get_next_execution_time(
        &self,
        last_run: Option<DateTime<Utc>>,
    ) -> Option<DateTime<Utc>> {
        match last_run {
            Some(last) => self.schedule.after(&last).next(),
            None => self.schedule.upcoming(Utc).next(),
        }
    }

    pub fn get_previous_execution_time(&self, before: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.schedule
            .upcoming(Utc)
            .take_while(|&time| time < before)
            .last()
    }

    pub fn validate_cron_expression(cron_expr: &str) -> SchedulerResult<()> {
        Schedule::from_str(cron_expr).map_err(|e| SchedulerError::InvalidCron {
            expr: cron_expr.to_string(),
            message: e.to_string(),
        })?;
        Ok(())
    }

    pub fn get_schedule_description(&self) -> String {
        format!("Cron schedule: {}", self.schedule)
    }

    // Additional methods from dispatcher version
    pub fn next_execution_time(&self, from: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.schedule.after(&from).next()
    }

    pub fn upcoming_times(&self, from: DateTime<Utc>, count: usize) -> Vec<DateTime<Utc>> {
        self.schedule.after(&from).take(count).collect()
    }

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

    pub fn get_frequency_description(&self) -> String {
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

    pub fn time_until_next_execution(&self, now: DateTime<Utc>) -> Option<Duration> {
        self.schedule.after(&now).next().map(|next| next - now)
    }

    pub fn will_execute_within(&self, from: DateTime<Utc>, duration: Duration) -> bool {
        if let Some(next_time) = self.schedule.after(&from).next() {
            next_time <= from + duration
        } else {
            false
        }
    }
}
