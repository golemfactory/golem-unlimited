mod connection;
mod context;
mod message;
mod router;
mod util;

mod error {
    error_chain!{

        errors {
            NotConnected
            MailBox
        }

    }

    use actix::MailboxError;

    impl From<MailboxError> for Error {
        fn from(e: MailboxError) -> Self {
            ErrorKind::MailBox.into()
        }
    }
}

pub use self::context::{RemotingContext, start_actor};