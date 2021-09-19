use x11rb::{rust_connection::{ConnectionError, ReplyError, ReplyOrIdError}, x11_utils::X11Error};

pub enum GrabError {
    TooManyRetriesError,
    ReplyOrIdError(ReplyOrIdError),
    ConnectionError(ConnectionError),
    Misc(X11Error)
}

impl From<ReplyOrIdError> for GrabError {
    fn from(e: ReplyOrIdError) -> Self {
        GrabError::ReplyOrIdError(e)
    }
}

impl From<ConnectionError> for GrabError {
    fn from(e: ConnectionError) -> Self {
        GrabError::ConnectionError(e)
    }
}

impl From<ReplyError> for GrabError {
    fn from(e: ReplyError) -> Self {
        match e {
            ReplyError::ConnectionError(err) => GrabError::ConnectionError(err),
            ReplyError::X11Error(err) => GrabError::Misc(err),
        }
    }
}

pub enum KeyboardError {
   ConnectionError(ConnectionError),
   NotFound,
   Misc(X11Error)
}

impl From<ConnectionError> for KeyboardError {
    fn from(e: ConnectionError) -> Self {
        Self::ConnectionError(e)
    }
}

impl From<ReplyError> for KeyboardError {
    fn from(e: ReplyError) -> Self {
        match e {
            ReplyError::ConnectionError(err) => Self::ConnectionError(err),
            ReplyError::X11Error(err) => Self::Misc(err),
        }
    }
}

#[derive(Debug)]
pub enum GenericConnectionError {
   ConnectionError(ConnectionError),
   NotFound,
   Misc(X11Error)
}

impl From<ConnectionError> for GenericConnectionError {
    fn from(e: ConnectionError) -> Self {
        Self::ConnectionError(e)
    }
}

impl From<ReplyError> for GenericConnectionError {
    fn from(e: ReplyError) -> Self {
        match e {
            ReplyError::ConnectionError(err) => Self::ConnectionError(err),
            ReplyError::X11Error(err) => Self::Misc(err),
        }
    }
}
