#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CliApprovalPolicy {
    Ask,
    Reject,
    Auto,
}
