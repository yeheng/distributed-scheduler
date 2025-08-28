//! # SQLx 数据库类型转换实现
//!
//! 本模块仅在启用 `sqlx-support` feature 时编译
//! 提供领域类型到数据库类型的转换支持

#[cfg(feature = "sqlx-support")]
use crate::entities::{TaskRunStatus, TaskStatus, WorkerStatus};

// TaskStatus SQLx 实现
#[cfg(feature = "sqlx-support")]
impl sqlx::Type<sqlx::Postgres> for TaskStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("VARCHAR")
    }
}

#[cfg(feature = "sqlx-support")]
impl sqlx::Type<sqlx::Sqlite> for TaskStatus {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <str as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
}

#[cfg(feature = "sqlx-support")]
impl<'r> sqlx::Decode<'r, sqlx::Postgres> for TaskStatus {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        match s {
            "ACTIVE" => Ok(TaskStatus::Active),
            "INACTIVE" => Ok(TaskStatus::Inactive),
            _ => Err(format!("Invalid task status: {s}").into()),
        }
    }
}

#[cfg(feature = "sqlx-support")]
impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for TaskStatus {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
        match s {
            "ACTIVE" => Ok(TaskStatus::Active),
            "INACTIVE" => Ok(TaskStatus::Inactive),
            _ => Err(format!("Invalid task status: {s}").into()),
        }
    }
}

#[cfg(feature = "sqlx-support")]
impl<'q> sqlx::Encode<'q, sqlx::Postgres> for TaskStatus {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let s = match self {
            TaskStatus::Active => "ACTIVE",
            TaskStatus::Inactive => "INACTIVE",
        };
        <&str as sqlx::Encode<sqlx::Postgres>>::encode(s, buf)
    }
}

#[cfg(feature = "sqlx-support")]
impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for TaskStatus {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let s = match self {
            TaskStatus::Active => "ACTIVE",
            TaskStatus::Inactive => "INACTIVE",
        };
        <&str as sqlx::Encode<sqlx::Sqlite>>::encode(s, buf)
    }
}

// TaskRunStatus SQLx 实现
#[cfg(feature = "sqlx-support")]
impl sqlx::Type<sqlx::Postgres> for TaskRunStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("VARCHAR")
    }
}

#[cfg(feature = "sqlx-support")]
impl sqlx::Type<sqlx::Sqlite> for TaskRunStatus {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <str as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
}

#[cfg(feature = "sqlx-support")]
impl<'r> sqlx::Decode<'r, sqlx::Postgres> for TaskRunStatus {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        match s {
            "PENDING" => Ok(TaskRunStatus::Pending),
            "DISPATCHED" => Ok(TaskRunStatus::Dispatched),
            "RUNNING" => Ok(TaskRunStatus::Running),
            "COMPLETED" => Ok(TaskRunStatus::Completed),
            "FAILED" => Ok(TaskRunStatus::Failed),
            "TIMEOUT" => Ok(TaskRunStatus::Timeout),
            _ => Err(format!("Invalid task run status: {s}").into()),
        }
    }
}

#[cfg(feature = "sqlx-support")]
impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for TaskRunStatus {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
        match s {
            "PENDING" => Ok(TaskRunStatus::Pending),
            "DISPATCHED" => Ok(TaskRunStatus::Dispatched),
            "RUNNING" => Ok(TaskRunStatus::Running),
            "COMPLETED" => Ok(TaskRunStatus::Completed),
            "FAILED" => Ok(TaskRunStatus::Failed),
            "TIMEOUT" => Ok(TaskRunStatus::Timeout),
            _ => Err(format!("Invalid task run status: {s}").into()),
        }
    }
}

#[cfg(feature = "sqlx-support")]
impl<'q> sqlx::Encode<'q, sqlx::Postgres> for TaskRunStatus {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let s = match self {
            TaskRunStatus::Pending => "PENDING",
            TaskRunStatus::Dispatched => "DISPATCHED",
            TaskRunStatus::Running => "RUNNING",
            TaskRunStatus::Completed => "COMPLETED",
            TaskRunStatus::Failed => "FAILED",
            TaskRunStatus::Timeout => "TIMEOUT",
        };
        <&str as sqlx::Encode<sqlx::Postgres>>::encode(s, buf)
    }
}

#[cfg(feature = "sqlx-support")]
impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for TaskRunStatus {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let s = match self {
            TaskRunStatus::Pending => "PENDING",
            TaskRunStatus::Dispatched => "DISPATCHED",
            TaskRunStatus::Running => "RUNNING",
            TaskRunStatus::Completed => "COMPLETED",
            TaskRunStatus::Failed => "FAILED",
            TaskRunStatus::Timeout => "TIMEOUT",
        };
        <&str as sqlx::Encode<sqlx::Sqlite>>::encode(s, buf)
    }
}

// WorkerStatus SQLx 实现
#[cfg(feature = "sqlx-support")]
impl sqlx::Type<sqlx::Postgres> for WorkerStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("VARCHAR")
    }
}

#[cfg(feature = "sqlx-support")]
impl sqlx::Type<sqlx::Sqlite> for WorkerStatus {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <&str as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
}

#[cfg(feature = "sqlx-support")]
impl<'r> sqlx::Decode<'r, sqlx::Postgres> for WorkerStatus {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        match s {
            "ALIVE" => Ok(WorkerStatus::Alive),
            "DOWN" => Ok(WorkerStatus::Down),
            _ => Err(format!("Invalid worker status: {s}").into()),
        }
    }
}

#[cfg(feature = "sqlx-support")]
impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for WorkerStatus {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
        match s {
            "ALIVE" => Ok(WorkerStatus::Alive),
            "DOWN" => Ok(WorkerStatus::Down),
            _ => Err(format!("Invalid worker status: {s}").into()),
        }
    }
}

#[cfg(feature = "sqlx-support")]
impl<'q> sqlx::Encode<'q, sqlx::Postgres> for WorkerStatus {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let s = match self {
            WorkerStatus::Alive => "ALIVE",
            WorkerStatus::Down => "DOWN",
        };
        <&str as sqlx::Encode<sqlx::Postgres>>::encode(s, buf)
    }
}

#[cfg(feature = "sqlx-support")]
impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for WorkerStatus {
    fn encode_by_ref(
        &self,
        args: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let s = match self {
            WorkerStatus::Alive => "ALIVE",
            WorkerStatus::Down => "DOWN",
        };
        <&str as sqlx::Encode<sqlx::Sqlite>>::encode(s, args)
    }
}
