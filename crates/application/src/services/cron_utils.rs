use chrono::{DateTime, Utc};
use cron::Schedule;
use std::str::FromStr;
use tracing::{debug, warn};

use scheduler_foundation::{SchedulerError, SchedulerResult};

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

    pub fn should_run(&self, now: DateTime<Utc>) -> bool {
        self.should_trigger(None, now)
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
}
