//! Classes: the selectable class record.

/// One selectable class.
pub struct ClassPlan {
    /// Trigger value (`/trigger dw.class set <n>`), 1-based.
    pub n: i32,
    /// Class id.
    pub class_id: String,
    /// Sanitized name for the apply function.
    pub safe: String,
}
