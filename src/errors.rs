use x11rb::{rust_connection::{ConnectionError, ReplyError, ReplyOrIdError}, x11_utils::X11Error};

pub enum XError {
    TooManyRetriesError,
    ReplyOrIdError(ReplyOrIdError),
    ConnectionError(ConnectionError),
    Misc(X11Error)
}

impl From<ReplyOrIdError> for XError {
    fn from(e: ReplyOrIdError) -> Self {
        XError::ReplyOrIdError(e)
    }
}

impl From<ConnectionError> for XError {
    fn from(e: ConnectionError) -> Self {
        XError::ConnectionError(e)
    }
}

impl From<ReplyError> for XError {
    fn from(e: ReplyError) -> Self {
        match e {
            ReplyError::ConnectionError(err) => XError::ConnectionError(err),
            ReplyError::X11Error(err) => XError::Misc(err),
        }
    }
}
