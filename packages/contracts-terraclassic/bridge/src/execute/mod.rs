//! Execute handlers for the CL8Y Bridge contract.
//!
//! This module contains all execute message handlers, organized by category:
//! - `outgoing` - Deposit handlers for outgoing transfers (lock/burn)
//! - `withdraw` - V2 withdrawal flow (submit, approve, cancel, uncancel, execute)
//! - `config` - Chain, token, operator, canceler, and rate limit management
//! - `admin` - Pause, unpause, admin transfer, and recovery operations

mod admin;
mod config;
mod outgoing;
mod withdraw;

pub use admin::*;
pub use config::*;
pub use outgoing::*;
pub use withdraw::*;
