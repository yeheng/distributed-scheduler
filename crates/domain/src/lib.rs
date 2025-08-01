//! Domain Layer
//! 
//! 业务领域层，包含核心业务逻辑和领域模型
//! 不依赖外部技术实现，体现依赖倒置原则

pub mod entities;
pub mod services;
pub mod repositories;
pub mod value_objects;
pub mod events;

// 重新导出核心类型
pub use entities::*;
pub use services::*;
pub use repositories::*;
pub use value_objects::*;
pub use events::*;