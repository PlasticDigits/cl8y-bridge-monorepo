//! Execute handlers for the CL8Y Bridge contract.
//!
//! This module contains all execute message handlers, organized by category:
//! - `outgoing` - Lock and Receive handlers for outgoing transfers
//! - `watchtower` - ApproveWithdraw, ExecuteWithdraw, Cancel, Reenable handlers
//! - `config` - Chain, token, operator, canceler, and rate limit management
//! - `admin` - Pause, unpause, admin transfer, and recovery operations

mod admin;
mod config;
mod outgoing;
mod watchtower;

pub use admin::*;
pub use config::*;
pub use outgoing::*;
pub use watchtower::*;
