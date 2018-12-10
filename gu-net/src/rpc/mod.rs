mod connection;
mod context;
mod message;
pub mod mock;
mod monitor;
pub mod peer;
mod registry;
pub mod remoting;
pub mod reply;
pub mod router;
mod util;
pub mod ws;

mod error {

    use quick_protobuf;

    error_chain! {

        errors {
            NotConnected
            MailBox(e : MailboxError)
            Canceled
            NoDestination
            BadFormat(s : String)
            Proto(e : quick_protobuf::Error)
        }

    }

    use actix::MailboxError;

    impl From<MailboxError> for Error {
        fn from(e: MailboxError) -> Self {
            ErrorKind::MailBox(e).into()
        }
    }

    use futures::Canceled;

    impl From<Canceled> for Error {
        fn from(e: Canceled) -> Self {
            ErrorKind::Canceled.into()
        }
    }

    impl From<quick_protobuf::Error> for Error {
        fn from(e: quick_protobuf::Error) -> Self {
            ErrorKind::Proto(e).into()
        }
    }
}

pub use self::{
    context::{start_actor, RemotingContext},
    error::Error as RpcError,
    message::{
        gen_destination_id, public_destination, DestinationId, EmitMessage, MessageId, RouteMessage,
    },
    registry::RemotingSystemService,
    remoting::{peer, PublicMessage},
    reply::ReplyRouter,
    router::MessageRouter,
};
