pub const DEFAULT_ROOT: &str =
    "/Users/xeed/Library/Mobile Documents/com~apple~CloudDocs/SOPs/email-inbox-processing";
pub const DEFAULT_GMAIL_QUERY: &str = "in:inbox";
pub const DEFAULT_TZ: &str = "America/Chicago";

pub const QUEUE_FIELDS: &[&str] = &[
    "status",
    "approval_state",
    "research_state",
    "read_state",
    "recommended_action",
    "terminal_action",
    "dashboard_anchor",
];
