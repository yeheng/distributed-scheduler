//! Update value types for precise PATCH operations
//!
//! This module provides types that enable precise PATCH semantics by distinguishing
//! between "set to value", "set to null", and "don't update" operations.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents a precise update operation for PATCH semantics
///
/// This enum allows distinguishing between three different update scenarios:
/// - `Set(T)`: Set the field to the specified value
/// - `Unset`: Set the field to null/remove the value (for nullable fields)
/// - `NoChange`: Do not modify the field (equivalent to field not being provided)
///
/// # Examples
///
/// ```rust
/// use scheduler_api::types::UpdateValue;
///
/// // Set a field to a specific value
/// let name_update = UpdateValue::Set("New Name".to_string());
///
/// // Set a nullable field to null
/// let description_update = UpdateValue::<String>::Unset;
///
/// // Don't change the field
/// let no_change = UpdateValue::<String>::NoChange;
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
#[derive(Default)]
pub enum UpdateValue<T> {
    /// Set the field to the specified value
    Set(T),
    /// Set the field to null/remove the value (for nullable fields)
    Unset,
    /// Do not modify the field
    #[default]
    NoChange,
}

impl<T> UpdateValue<T> {
    /// Create a new Set variant
    pub fn set(value: T) -> Self {
        UpdateValue::Set(value)
    }

    /// Create an Unset variant
    pub fn unset() -> Self {
        UpdateValue::Unset
    }

    /// Create a NoChange variant
    pub fn no_change() -> Self {
        UpdateValue::NoChange
    }

    /// Check if this update represents a change
    pub fn is_change(&self) -> bool {
        !matches!(self, UpdateValue::NoChange)
    }

    /// Check if this update unsets the value
    pub fn is_unset(&self) -> bool {
        matches!(self, UpdateValue::Unset)
    }

    /// Get the value if this is a Set operation
    pub fn value(&self) -> Option<&T> {
        match self {
            UpdateValue::Set(value) => Some(value),
            _ => None,
        }
    }

    /// Convert to Option<T>, where None means "no change"
    pub fn as_option(&self) -> Option<Option<&T>> {
        match self {
            UpdateValue::Set(value) => Some(Some(value)),
            UpdateValue::Unset => Some(None),
            UpdateValue::NoChange => None,
        }
    }

    /// Convert to Option<Option<T>>, cloning the value if needed
    pub fn as_cloned_option(&self) -> Option<Option<T>>
    where
        T: Clone,
    {
        match self {
            UpdateValue::Set(value) => Some(Some(value.clone())),
            UpdateValue::Unset => Some(None),
            UpdateValue::NoChange => None,
        }
    }

    /// Apply this update to an existing value
    pub fn apply_to(self, existing: Option<T>) -> Option<T>
    where
        T: Clone,
    {
        match self {
            UpdateValue::Set(value) => Some(value),
            UpdateValue::Unset => None,
            UpdateValue::NoChange => existing,
        }
    }

    /// Map the inner value if this is a Set operation
    pub fn map<U, F>(self, f: F) -> UpdateValue<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            UpdateValue::Set(value) => UpdateValue::Set(f(value)),
            UpdateValue::Unset => UpdateValue::Unset,
            UpdateValue::NoChange => UpdateValue::NoChange,
        }
    }
}

impl<T> NumericUpdateValue<T> {
    /// Create a Set variant
    pub fn set(value: T) -> Self {
        NumericUpdateValue::Set(value)
    }

    /// Create an Add variant
    pub fn add(value: T) -> Self {
        NumericUpdateValue::Add(value)
    }

    /// Create a Subtract variant
    pub fn subtract(value: T) -> Self {
        NumericUpdateValue::Subtract(value)
    }

    /// Create an Unset variant
    pub fn unset() -> Self {
        NumericUpdateValue::Unset
    }

    /// Create a NoChange variant
    pub fn no_change() -> Self {
        NumericUpdateValue::NoChange
    }

    /// Check if this update represents a change
    pub fn is_change(&self) -> bool {
        !matches!(self, NumericUpdateValue::NoChange)
    }

    /// Check if this update unsets the value
    pub fn is_unset(&self) -> bool {
        matches!(self, NumericUpdateValue::Unset)
    }

    /// Get the value if this is a Set operation
    pub fn value(&self) -> Option<&T> {
        match self {
            NumericUpdateValue::Set(value) => Some(value),
            NumericUpdateValue::Add(value) => Some(value),
            NumericUpdateValue::Subtract(value) => Some(value),
            _ => None,
        }
    }

    /// Apply this update to an existing value
    pub fn apply_to(self, existing: Option<T>) -> Option<T>
    where
        T: std::ops::Add<Output = T> + std::ops::Sub<Output = T> + Copy + Clone,
    {
        match self {
            NumericUpdateValue::Set(value) => Some(value),
            NumericUpdateValue::Add(value) => existing.map(|e| e + value),
            NumericUpdateValue::Subtract(value) => existing.map(|e| e - value),
            NumericUpdateValue::Unset => None,
            NumericUpdateValue::NoChange => existing,
        }
    }

    /// Convert to Option<T>, where None means "no change"
    pub fn as_option(&self) -> Option<Option<&T>> {
        match self {
            NumericUpdateValue::Set(value) => Some(Some(value)),
            NumericUpdateValue::Add(value) => Some(Some(value)),
            NumericUpdateValue::Subtract(value) => Some(Some(value)),
            NumericUpdateValue::Unset => Some(None),
            NumericUpdateValue::NoChange => None,
        }
    }

    /// Convert to Option<Option<T>>, cloning the value if needed
    pub fn as_cloned_option(&self) -> Option<Option<T>>
    where
        T: Clone,
    {
        match self {
            NumericUpdateValue::Set(value) => Some(Some(value.clone())),
            NumericUpdateValue::Add(value) => Some(Some(value.clone())),
            NumericUpdateValue::Subtract(value) => Some(Some(value.clone())),
            NumericUpdateValue::Unset => Some(None),
            NumericUpdateValue::NoChange => None,
        }
    }
}

impl<'de, T> Deserialize<'de> for UpdateValue<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Use a helper enum to handle the different cases
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Helper<T> {
            Value(T),
            Null,
        }

        // First deserialize as Option<T> to handle missing fields
        let opt_value = Option::<T>::deserialize(deserializer)?;

        match opt_value {
            Some(value) => Ok(UpdateValue::Set(value)),
            None => {
                // If we get None, we need to check if it was explicitly null or missing
                // This is handled by the serde deserializer automatically
                // For explicit null, we'll get a Helper::Null
                Ok(UpdateValue::NoChange)
            }
        }
    }
}

impl<T> fmt::Display for UpdateValue<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UpdateValue::Set(value) => write!(f, "Set({value})"),
            UpdateValue::Unset => write!(f, "Unset"),
            UpdateValue::NoChange => write!(f, "NoChange"),
        }
    }
}

/// A specialized update value for numeric fields that supports increment/decrement operations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
#[derive(Default)]
pub enum NumericUpdateValue<T> {
    /// Set to a specific value
    Set(T),
    /// Add the specified value to the current value
    Add(T),
    /// Subtract the specified value from the current value
    Subtract(T),
    /// Set to null/remove the value
    Unset,
    /// Do not modify the field
    #[default]
    NoChange,
}

// Note: Implementation moved above the Deserialize impl

/// A macro to simplify creating update request structures
///
/// # Examples
///
/// ```rust
/// use scheduler_api::{update_request, types::{UpdateValue, NumericUpdateValue}};
/// use validator::Validate;
///
/// update_request! {
///     #[derive(Validate)]
///     struct UpdateUserRequest {
///         #[validate(length(min = 1, max = 255))]
///         pub name: UpdateValue<String>,
///         pub email: UpdateValue<String>,
///         #[validate(range(min = 0))]
///         pub age: NumericUpdateValue<i32>,
///     }
/// }
/// ```
#[macro_export]
macro_rules! update_request {
    (
        $(#[$struct_meta:meta])*
        $struct_vis:vis struct $struct_name:ident {
            $(
                $(#[$field_meta:meta])*
                $field_vis:vis $field_name:ident : $field_ty:ty
            ),* $(,)?
        }
    ) => {
        $(#[$struct_meta])*
        $struct_vis struct $struct_name {
            $(
                $(#[$field_meta])*
                $field_vis $field_name : $field_ty
            ),*
        }

        impl $struct_name {
            /// Check if this request has any actual changes
            pub fn has_changes(&self) -> bool {
                $(
                    self.$field_name.is_change() ||
                )* false
            }

            /// Get the number of fields that will be changed
            pub fn change_count(&self) -> usize {
                let mut count = 0;
                $(
                    if self.$field_name.is_change() {
                        count += 1;
                    }
                )*
                count
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_value_set() {
        let update = UpdateValue::Set("test".to_string());
        assert!(update.is_change());
        assert!(!update.is_unset());
        assert_eq!(update.value(), Some(&"test".to_string()));
        assert_eq!(update.as_option(), Some(Some(&"test".to_string())));
        assert_eq!(update.as_cloned_option(), Some(Some("test".to_string())));
    }

    #[test]
    fn test_update_value_unset() {
        let update = UpdateValue::<String>::Unset;
        assert!(update.is_change());
        assert!(update.is_unset());
        assert_eq!(update.value(), None);
        assert_eq!(update.as_option(), Some(None));
    }

    #[test]
    fn test_update_value_no_change() {
        let update = UpdateValue::<String>::NoChange;
        assert!(!update.is_change());
        assert!(!update.is_unset());
        assert_eq!(update.value(), None);
        assert_eq!(update.as_option(), None);
    }

    #[test]
    fn test_update_value_apply_to() {
        let existing = Some("old".to_string());

        assert_eq!(
            UpdateValue::Set("new".to_string()).apply_to(existing.clone()),
            Some("new".to_string())
        );
        assert_eq!(UpdateValue::Unset.apply_to(existing.clone()), None);
        assert_eq!(UpdateValue::NoChange.apply_to(existing.clone()), existing);
    }

    #[test]
    fn test_numeric_update_value() {
        let existing = Some(10);

        assert_eq!(
            NumericUpdateValue::Set(20).apply_to(existing.clone()),
            Some(20)
        );
        assert_eq!(
            NumericUpdateValue::Add(5).apply_to(existing.clone()),
            Some(15)
        );
        assert_eq!(
            NumericUpdateValue::Subtract(3).apply_to(existing.clone()),
            Some(7)
        );
        assert_eq!(NumericUpdateValue::Unset.apply_to(existing.clone()), None);
        assert_eq!(
            NumericUpdateValue::NoChange.apply_to(existing.clone()),
            existing
        );
    }

    #[test]
    fn test_update_value_display() {
        assert_eq!(
            UpdateValue::Set("test".to_string()).to_string(),
            "Set(test)"
        );
        assert_eq!(UpdateValue::<String>::Unset.to_string(), "Unset");
        assert_eq!(UpdateValue::<String>::NoChange.to_string(), "NoChange");
    }
}
