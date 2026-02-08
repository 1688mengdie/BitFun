/**
 * Function Agents module
 *
 * Provides various function agents for automating specific tasks
 */

#[path = "git-func-agent/mod.rs"]
pub mod git_func_agent;

#[path = "startchat-func-agent/mod.rs"]
pub mod startchat_func_agent;

pub use git_func_agent::GitFunctionAgent;
pub use startchat_func_agent::StartchatFunctionAgent;

pub use git_func_agent::{
    CommitMessage,
    CommitMessageOptions,
    CommitFormat,
    CommitType,
};

pub use startchat_func_agent::{
    WorkStateAnalysis,
    WorkStateOptions,
    GreetingMessage,
    CurrentWorkState,
    GitWorkState,
    PredictedAction,
    QuickAction,
};
