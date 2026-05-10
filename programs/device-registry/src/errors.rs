use anchor_lang::prelude::*;

#[error_code]
pub enum ColdchainError {
    #[msg("Device is already assigned to a shipment")]
    DeviceAlreadyAssigned,
    #[msg("Device is not currently assigned")]
    DeviceNotAssigned,
    #[msg("Assignment has already been ended")]
    AssignmentAlreadyEnded,
    #[msg("Assignment account does not match device's current assignment")]
    AssignmentMismatch,
    #[msg("Invalid shipment status transition")]
    InvalidStatusTransition,
    #[msg("Shipment is closed and cannot accept new operations")]
    ShipmentClosed,
    #[msg("Cannot assign device to a shipment that is not in transit or created")]
    ShipmentNotAcceptingDevices,
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
}
