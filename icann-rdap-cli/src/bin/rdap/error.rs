use std::process::{ExitCode, Termination};

use icann_rdap_client::iana::IanaResponseError;
use icann_rdap_client::RdapClientError;
use minus::MinusError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RdapCliError {
    #[error("No errors encountered")]
    Success,
    #[error(transparent)]
    RdapClient(#[from] RdapClientError),
    #[error(transparent)]
    Termimad(#[from] termimad::Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    Minus(#[from] MinusError),
    #[error("Unknown output type")]
    UnknownOutputType,
    #[error("RDAP response failed checks.")]
    ErrorOnChecks,
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Iana(#[from] IanaResponseError),
    #[error("Invalid IANA bootsrap file")]
    InvalidBootstrap,
    #[error("Bootstrap not found")]
    BootstrapNotFound,
    #[error("No registrar found")]
    NoRegistrarFound,
    #[error("No registry found")]
    NoRegistryFound,
}

impl Termination for RdapCliError {
    fn report(self) -> std::process::ExitCode {
        let exit_code: u8 = match self {
            // Success
            Self::Success => 0,

            // Internal Errors
            Self::Termimad(_) => 10,
            Self::Minus(_) => 11,

            // I/O Errors
            Self::IoError(_) => 40,

            // RDAP Errors
            Self::Json(_) => 100,
            Self::Iana(_) => 101,
            Self::InvalidBootstrap => 102,
            Self::BootstrapNotFound => 103,
            Self::NoRegistrarFound => 104,
            Self::NoRegistryFound => 105,

            // User Errors
            Self::UnknownOutputType => 200,
            Self::ErrorOnChecks => 201,

            // RDAP Client Errrors
            Self::RdapClient(e) => match e {
                // I/O Errors
                RdapClientError::Client(_) => 42,
                RdapClientError::IoError(_) => 43,

                // RDAP Server Errors
                RdapClientError::Response(_) => 60,
                RdapClientError::ParsingError(_) => 62,
                RdapClientError::Json(_) => 63,

                // Bootstrap Errors
                RdapClientError::BootstrapUnavailable => 70,
                RdapClientError::BootstrapError(_) => 71,
                RdapClientError::IanaResponse(_) => 72,

                // User Errors
                RdapClientError::InvalidQueryValue => 202,
                RdapClientError::AmbiquousQueryType => 203,
                RdapClientError::DomainNameError(_) => 204,

                // Internal Errors
                RdapClientError::Poison => 250,
                // _ => 255,
            },
        };
        ExitCode::from(exit_code)
    }
}
