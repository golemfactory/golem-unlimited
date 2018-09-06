mod connection;
mod context;
mod message;
pub mod mock;
mod reply;
pub mod router;
mod util;
pub mod ws;

mod error {
    error_chain!{

        errors {
            NotConnected
            MailBox
            Canceled
            NoDestination
            BadFormat(s : String)
        }

    }

    use actix::MailboxError;

    impl From<MailboxError> for Error {
        fn from(e: MailboxError) -> Self {
            ErrorKind::MailBox.into()
        }
    }

    use futures::Canceled;

    impl From<Canceled> for Error {
        fn from(e: Canceled) -> Self {
            ErrorKind::Canceled.into()
        }
    }
}

pub use self::context::{start_actor, RemotingContext};
pub use self::error::Error as RpcError;
pub use self::message::{
    gen_destination_id, public_destination, DestinationId, EmitMessage, MessageId, RouteMessage,
};
pub use self::reply::ReplyRouter;
pub use self::router::MessageRouter;
