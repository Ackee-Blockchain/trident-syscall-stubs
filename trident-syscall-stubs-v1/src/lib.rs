pub mod syscall_stubs;
pub use syscall_stubs::*;

pub mod invoke_context;
pub use invoke_context::*;

// Trait to convert between types that are not directly compatible
pub trait TridentTryFrom<T>: Sized {
    /// The type returned in the event of a conversion error.
    type Error;

    /// Performs the conversion.
    fn try_from_custom(value: T) -> Result<Self, Self::Error>;
}
