//! Platform-neutral online media services.
//!
//! This module deliberately contains no WebView2, Windows path, Android SDK,
//! keyring or vendor UI code. Platform shells resolve a `CredentialRef` and
//! construct a provider without exposing the secret to research artifacts.

pub mod models;
pub mod providers;
pub mod transcription;

#[allow(unused_imports)]
pub use models::{
    AudioFormat, AudioInput, CredentialRef, TranscriptionError, TranscriptionRequest,
    TranscriptionResult, TranscriptionSegment,
};
#[allow(unused_imports)]
pub use transcription::{
    provider_descriptor, ProviderAvailability, ProviderDescriptor, ProviderRegistry,
    TranscriptionProvider,
};
