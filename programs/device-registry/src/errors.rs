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
    #[msg("Shipment is closed and cannot accept new operations")]
    ShipmentClosed,
    #[msg("Submitter is not the authority of the device referenced by the assignment")]
    UnauthorizedSubmitter,
    #[msg("Assignment's device field does not match the provided device account")]
    AssignmentDeviceMismatch,
    #[msg("Assignment has been ended; cannot submit proofs against it")]
    AssignmentEnded,
    #[msg("Unauthorized: signer does not match the required authority")]
    Unauthorized,
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
    #[msg("Shipment must be closed before it can be attested")]
    ShipmentNotClosed,
    #[msg("Shipment has reached its maximum attestation capacity")]
    AttestationCapacityExceeded,
}
