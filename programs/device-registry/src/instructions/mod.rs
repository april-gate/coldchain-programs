#![allow(ambiguous_glob_reexports)]

pub mod register_device;
pub mod create_shipment;
pub mod update_shipment_status;
pub mod assign_device;
pub mod end_assignment;
pub mod submit_proof;

// Glob re-exports are required so that #[derive(Accounts)]'s auto-generated
// companion modules (__client_accounts_*, __cpi_client_accounts_*) are
// reachable from the crate root, which is where #[program] expects them.
//
// The "ambiguous glob re-exports" warning about `handler` is harmless —
// each handler is only called via its fully-qualified path
// (e.g. `instructions::register_device::handler`) from lib.rs.
pub use register_device::*;
pub use create_shipment::*;
pub use update_shipment_status::*;
pub use assign_device::*;
pub use end_assignment::*;
pub use submit_proof::*;